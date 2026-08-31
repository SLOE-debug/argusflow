# ArgusFlow Workflow 泛化改造方案

## 目标
把 ArgusFlow 从“微信自动化程序 + workflow 编排”改成真正的**通用视觉 Workflow Runtime**。

核心原则只有一句：

> **程序实现“怎么看、怎么操作”；Workflow 决定“现在是什么状态、接下来走哪条路”。**

---

## 1. 重新划分程序与 Workflow 的边界

### 程序层只保留通用原子能力

例如：

- `capture / crop`
- `ocr / text_exists`
- `template_match / locate / exists`
- `click / type / key / scroll`
- `wait`

统一返回结构化 `Observation`，例如：

```yaml
matched: true
confidence: 0.94
text: "hello"
bbox: [x, y, w, h]
data: {}
```

程序层**不要再出现**这类业务语义：

- `wechat_send_success`
- `wechat_message_list`
- `is_send_failed_then_retry`

这些都应该由 Workflow 表达。

---

## 2. 给 Workflow 增加“视觉状态”这一层

Workflow 不直接实现 CV 算法，而是组合通用 detector，声明“什么现象代表什么状态”。

```yaml
states:
  send_result:
    cases:
      - value: failed
        when: { exists: send_failed_icon }

      - value: sent
        when:
          all:
            - text_exists: { region: chat_history, text: "${message}" }
            - not: { exists: send_failed_icon }

      - value: unknown
```

然后流程只消费这个状态：

```yaml
- observe: send_result
  save_as: result

- switch: "${result}"
  cases:
    sent: done
    failed: retry_send
    unknown: verify_again
```

这样“发送成功”的定义本身也不再写死在程序里。

---

## 3. Workflow Runtime 必须补齐的能力

优先实现下面几个一等节点/语义：

- `observe`：执行视觉观察并保存结果
- `all / any / not / compare`：组合判断条件
- `if / switch`：情况分支
- `retry + timeout`：重试与超时
- `loop`：循环等待状态变化
- `fallback`：检测失败后的备用路径
- `call`：调用可复用 subflow
- `success / fail`：显式结束状态

同时加入运行时上下文：

```text
ctx.vars
ctx.observations
ctx.last_action
ctx.last_error
```

节点只读写 Context，不彼此硬耦合。

---

## 4. 微信相关内容拆成 Profile + Workflow

建议结构：

```text
profiles/
  wechat.yaml          # 窗口、区域、锚点、模板等视觉描述

assets/wechat/
  send_failed.png
  ...

workflows/wechat/
  open_chat.yaml
  send_message.yaml    # 状态、分支、重试、恢复逻辑
```

`profile` 只描述“微信长什么样”；`workflow` 描述“遇到这些现象怎么办”。

如果某个复杂识别必须写代码，也只能做成**纯 Detector Plugin**：输入截图，输出 Observation；它不能在内部重试、跳转或决定业务流程。

---

## 5. 实施顺序

1. 盘点现有微信代码中的所有 `状态判断 + if/else + retry`。
2. 能泛化的识别动作下沉成视觉 primitive，并统一输出 `Observation`。
3. 在 core/runtime 中实现 `observe / predicate / switch / retry / timeout / call`。
4. 把“发送成功、消息列表、发送失败、重新进入聊天”等逻辑逐个迁移到 Workflow。
5. 最后用 **飞书 / Telegram / Discord 任意一个第二应用**验证：只新增 `profile + assets + workflow`，不修改 runtime/core。

---

## 最终验收标准

**新增一个聊天软件时，如果还需要修改 ArgusFlow 引擎代码，说明抽象仍然不够。**

最终模型应该是：

```text
ArgusFlow
= Visual Primitives
+ Declarative Visual States
+ Workflow State Machine
+ Runtime Context
```

你真正有价值的部分不是“微信自动化”，而是一个可以让用户**用 Workflow 自己定义视觉状态机**的执行引擎。
