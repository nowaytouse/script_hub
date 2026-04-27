import os
import re

CATEGORIES = {
    "amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "head_expanse": "『 🔝 Head Expanse › 首端扩域 』",
    "narrow_pierce": "『 🎯 Narrow Pierce › 窄域穿刺 』"
}

ROOT_DIRS = ["module/surge(main)", "module/shadowrocket"]

def fix_categories():
    for root_dir in ROOT_DIRS:
        if not os.path.exists(root_dir):
            continue
        for cat_dir_name, cat_string in CATEGORIES.items():
            dir_path = os.path.join(root_dir, cat_dir_name)
            if not os.path.exists(dir_path):
                continue
            
            for filename in os.listdir(dir_path):
                if filename.endswith((".sgmodule", ".module")):
                    file_path = os.path.join(dir_path, filename)
                    with open(file_path, "r", encoding="utf-8") as f:
                        content = f.read()
                    
                    # For Surge modules
                    if filename.endswith(".sgmodule"):
                        if "#!category=" in content:
                            new_content = re.sub(r'^#!category=.*', f'#!category={cat_string}', content, flags=re.MULTILINE)
                        else:
                            # Insert after #!name if exists, else at top
                            if "#!name=" in content:
                                new_content = re.sub(r'^(#!name=.*)', f'\\1\n#!category={cat_string}', content, flags=re.MULTILINE)
                            else:
                                new_content = f"#!category={cat_string}\n" + content
                    
                    # For Shadowrocket modules
                    else:
                        if "# category:" in content:
                            new_content = re.sub(r'^# category:.*', f'# category: {cat_string}', content, flags=re.MULTILINE)
                        else:
                            new_content = f"# category: {cat_string}\n" + content
                    
                    if new_content != content:
                        with open(file_path, "w", encoding="utf-8") as f:
                            f.write(new_content)
                        print(f"Fixed category for: {file_path}")

if __name__ == "__main__":
    fix_categories()
