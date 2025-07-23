import json

with open("crates/valence_generated/extracted/blocks.json", "r", encoding="utf-8") as f:
    data = json.load(f)

for block in data["blocks"]:
    if "item_id" in block and isinstance(block["item_id"], int):
        block["item_id"] = f"minecraft:{block['name']}"

with open("crates/valence_generated/extracted/blocks.json", "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)

