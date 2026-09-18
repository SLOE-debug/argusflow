"""按显式映射迁移中文 AQL 词法转换，重复运行不覆盖已有目标。"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MOVES = {
    f"crates/argusflow-aql-wasm/src/localization/{name}.rs":
    f"crates/argusflow-aql/src/localization/{name}.rs"
    for name in ("mod", "dictionary", "translation")
}

for source, destination in MOVES.items():
    source_path, destination_path = ROOT / source, ROOT / destination
    if not source_path.exists():
        if not destination_path.exists():
            raise FileNotFoundError(source_path)
        continue
    if destination_path.exists():
        raise FileExistsError(destination_path)
    destination_path.parent.mkdir(parents=True, exist_ok=True)
    source_path.rename(destination_path)
