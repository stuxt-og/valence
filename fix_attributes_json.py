import json

with open("crates/valence_generated/extracted/attributes.json", "r", encoding="utf-8") as f:
    data = json.load(f)

for key, attr in data.items():
    if "id" in attr and isinstance(attr["id"], int):
        attr["id"] = f"minecraft:{key}"

with open("crates/valence_generated/extracted/attributes.json", "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)

