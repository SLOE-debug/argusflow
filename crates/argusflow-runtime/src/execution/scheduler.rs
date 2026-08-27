use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Weak},
};

use argusflow_core::ResourceId;
use tokio::sync::{Mutex, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

/// 跨 RunWorld 仲裁外部副作用的稳定资源键。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceAccessKey {
    /// 获取后由 ResourceTable 分配的运行时资源身份。
    Runtime(ResourceId),
    /// 获取前即可确定的外部对象身份，例如应用 EXE 与窗口匹配契约。
    External(String),
    /// 不依赖具体实例的全局副作用域，例如当前前台 UI。
    Global(String),
}

impl ResourceAccessKey {
    /// 创建外部资源稳定身份。
    pub fn external(value: impl Into<String>) -> Self {
        Self::External(value.into())
    }

    /// 创建宿主级全局资源身份。
    pub fn global(value: impl Into<String>) -> Self {
        Self::Global(value.into())
    }
}

/// 一个节点对资源的访问模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceAccessMode {
    /// 不改变外部状态的并发安全读取。
    Read,
    /// 会改变外部状态或无法证明并发安全的独占访问。
    Exclusive,
}

/// 单个节点准备阶段声明的资源访问项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceAccess {
    /// 需要参与冲突检测的稳定资源身份。
    pub key: ResourceAccessKey,
    /// 读取或独占语义。
    pub mode: ResourceAccessMode,
}

/// PreparedNode 冻结的只读资源访问集合。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessSet {
    /// 同一节点的资源访问声明；Scheduler 会排序、合并并按稳定顺序加锁。
    pub resources: Vec<ResourceAccess>,
}

impl AccessSet {
    /// 创建单个资源的读取集合。
    pub fn read(key: ResourceAccessKey) -> Self {
        Self {
            resources: vec![ResourceAccess {
                key,
                mode: ResourceAccessMode::Read,
            }],
        }
    }

    /// 创建单个资源的独占集合。
    pub fn exclusive(key: ResourceAccessKey) -> Self {
        Self {
            resources: vec![ResourceAccess {
                key,
                mode: ResourceAccessMode::Exclusive,
            }],
        }
    }
}

/// 跨运行共享的资源冲突仲裁器。
#[derive(Default)]
pub(crate) struct ResourceScheduler {
    /// 每个仍在执行的稳定资源键拥有一把异步读写锁；弱引用避免运行结束后积累资源键。
    locks: Mutex<HashMap<ResourceAccessKey, Weak<RwLock<()>>>>,
}

impl ResourceScheduler {
    /// 按稳定键顺序获取全部访问权，避免多资源节点出现锁顺序死锁。
    pub(crate) async fn acquire(&self, access: AccessSet) -> ResourceAccessGuard {
        let normalized = access.resources.into_iter().fold(
            BTreeMap::<ResourceAccessKey, ResourceAccessMode>::new(),
            |mut entries, resource| {
                entries
                    .entry(resource.key)
                    .and_modify(|mode| {
                        if resource.mode == ResourceAccessMode::Exclusive {
                            *mode = ResourceAccessMode::Exclusive;
                        }
                    })
                    .or_insert(resource.mode);
                entries
            },
        );
        let mut guards = Vec::with_capacity(normalized.len());
        for (key, mode) in normalized {
            let lock = {
                let mut locks = self.locks.lock().await;
                // 下一次调度时顺手回收所有已没有执行者持有的资源键。
                locks.retain(|_, lock| lock.strong_count() > 0);
                if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
                    lock
                } else {
                    let lock = Arc::new(RwLock::new(()));
                    locks.insert(key, Arc::downgrade(&lock));
                    lock
                }
            };
            guards.push(match mode {
                ResourceAccessMode::Read => HeldAccess::Read(lock.read_owned().await),
                ResourceAccessMode::Exclusive => HeldAccess::Exclusive(lock.write_owned().await),
            });
        }
        ResourceAccessGuard { guards }
    }
}

/// 一次节点执行期间持有的全部资源访问权。
pub(crate) struct ResourceAccessGuard {
    /// 字段只负责把异步锁 guard 的生命周期延长到节点执行完成。
    guards: Vec<HeldAccess>,
}

impl Drop for ResourceAccessGuard {
    fn drop(&mut self) {
        // 显式读取字段，说明这些 guard 的唯一职责就是随集合一起释放。
        let _held_resource_count = self.guards.len();
    }
}

/// 读取和独占异步 guard 的统一所有权容器。
enum HeldAccess {
    /// 允许同一资源的其它读取节点并行。
    Read(OwnedRwLockReadGuard<()>),
    /// 阻止同一资源的其它读取或写入节点。
    Exclusive(OwnedRwLockWriteGuard<()>),
}

impl Drop for HeldAccess {
    fn drop(&mut self) {
        match self {
            Self::Read(guard) => {
                let _ = guard;
            }
            Self::Exclusive(guard) => {
                let _ = guard;
            }
        }
    }
}
