//! 验收专属无 UI 进程；可创建一个专属后代来验证 Job 所有权。
mod process_fixture_behavior;
fn main() {
    process_fixture_behavior::run();
}
