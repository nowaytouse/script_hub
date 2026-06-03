module_path = "/Users/nyamiiko/Downloads/GitHub/script_hub/modules/surge/head_expanse/🛡️ PROMAX AdBlock Helper (DNS & Firewall).sgmodule"
with open(module_path, 'r') as f:
    module_content = f.read()

if "[Host]" not in module_content:
    module_content = module_content.replace("[Rule]", "[Host]\n\n[Rule]")
    with open(module_path, 'w') as f:
        f.write(module_content)
    print("Inserted [Host]")
else:
    print("[Host] already exists")
