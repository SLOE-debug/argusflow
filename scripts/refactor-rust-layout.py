"""按显式映射整理 argusflow-runtime Rust 模块布局。

脚本只移动列出的文件并替换列出的 crate 路径，不尝试用正则理解 Rust 语义。
默认执行 dry-run；传入 --apply 后才会写入文件系统。
"""

from __future__ import annotations

import argparse
import shutil
from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parent.parent
RUNTIME_ROOT = WORKSPACE_ROOT / "crates" / "argusflow-runtime"


MOVE_MAP = {
    "src/engine.rs": "src/execution/engine.rs",
    "src/dispatcher.rs": "src/execution/dispatcher.rs",
    "src/execution_events.rs": "src/execution/execution_events.rs",
    "src/node_execution.rs": "src/execution/node_execution.rs",
    "src/run_context.rs": "src/execution/run_context.rs",
    "src/run_inputs.rs": "src/execution/run_inputs.rs",
    "src/scheduler.rs": "src/execution/scheduler.rs",
    "src/validator.rs": "src/validation/validator.rs",
    "src/validation_graph.rs": "src/validation/validation_graph.rs",
    "src/validation_references.rs": "src/validation/validation_references.rs",
    "src/component_expander.rs": "src/component/component_expander.rs",
    "src/component_registry.rs": "src/component/component_registry.rs",
    "src/component_rewrite.rs": "src/component/component_rewrite.rs",
    "src/resource_cleanup.rs": "src/resource/resource_cleanup.rs",
    "src/resource_table.rs": "src/resource/resource_table.rs",
    "src/command.rs": "src/command/mod.rs",
    "src/command_job.rs": "src/command/command_job.rs",
}


PATH_REWRITES = {
    "crate::command_job": "crate::command::command_job",
    "crate::component_rewrite": "crate::component::component_rewrite",
    "crate::execution_events": "crate::execution::execution_events",
    "crate::run_inputs": "crate::execution::run_inputs",
    "crate::scheduler": "crate::execution::scheduler",
    "crate::validation_graph": "crate::validation::validation_graph",
    "crate::validation_references": "crate::validation::validation_references",
    "crate::validator": "crate::validation::validator",
    "crate::resource_cleanup": "crate::resource::resource_cleanup",
    "crate::resource_table": "crate::resource::resource_table",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--apply",
        action="store_true",
        help="应用显式移动和路径替换；省略时只打印 dry-run 报告。",
    )
    return parser.parse_args()


def resolve(relative_path: str) -> Path:
    return RUNTIME_ROOT / relative_path


def validate_manifest() -> list[tuple[Path, Path]]:
    pending: list[tuple[Path, Path]] = []
    sources: set[Path] = set()
    targets: set[Path] = set()
    for source_name, target_name in MOVE_MAP.items():
        source = resolve(source_name)
        target = resolve(target_name)
        if source in sources or target in targets:
            raise ValueError(f"重复的 Rust move 映射: {source_name} -> {target_name}")
        if not source.exists() and not target.exists():
            raise FileNotFoundError(f"Rust move 源和目标都不存在: {source_name} -> {target_name}")
        if source.exists() and target.exists():
            raise FileExistsError(f"Rust move 源和目标同时存在: {source_name} -> {target_name}")
        if source.exists():
            pending.append((source, target))
        sources.add(source)
        targets.add(target)
    return pending


def rust_files() -> list[Path]:
    return sorted(RUNTIME_ROOT.glob("src/**/*.rs"))


def rewrite_content(content: str) -> str:
    for old_path, new_path in PATH_REWRITES.items():
        content = content.replace(old_path, new_path)
    return content


def main() -> None:
    args = parse_args()
    pending_moves = validate_manifest()
    print(
        f"{'Applying' if args.apply else 'Dry-run'} "
        f"{len(pending_moves)} pending Rust moves ({len(MOVE_MAP)} manifest entries)."
    )
    for source, target in pending_moves:
        print(f"  {source.relative_to(WORKSPACE_ROOT)} -> {target.relative_to(WORKSPACE_ROOT)}")

    changed_files = 0
    for file_path in rust_files():
        current = file_path.read_text(encoding="utf-8")
        rewritten = rewrite_content(current)
        if rewritten != current:
            changed_files += 1
            if args.apply:
                file_path.write_text(rewritten, encoding="utf-8")
    print(f"Planned {changed_files} explicit Rust path rewrites.")

    if not args.apply:
        return

    for source, target in pending_moves:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(source, target)
    validate_no_old_paths()
    print("Rust module moves and explicit path rewrites completed.")


def validate_no_old_paths() -> None:
    """确认 runtime 源码中没有残留本轮显式迁移前的 crate 路径。"""
    leftovers = []
    for file_path in rust_files():
        content = file_path.read_text(encoding="utf-8")
        for old_path in PATH_REWRITES:
            if old_path in content:
                leftovers.append(f"{file_path.relative_to(WORKSPACE_ROOT)} -> {old_path}")
    if leftovers:
        raise ValueError(
            "Rust 迁移后仍存在旧路径残留:\n" + "\n".join(leftovers)
        )


if __name__ == "__main__":
    main()
