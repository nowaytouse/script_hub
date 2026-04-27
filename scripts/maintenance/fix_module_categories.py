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
                        lines = f.readlines()
                    
                    new_lines = []
                    is_surge = filename.endswith(".sgmodule")
                    
                    # 1. 移除所有现有的 category 行
                    for line in lines:
                        if is_surge:
                            if not line.strip().startswith("#!category"):
                                new_lines.append(line)
                        else:
                            if not line.strip().startswith("# category:"):
                                new_lines.append(line)
                    
                    # 2. 寻找插入点并注入唯一的标准 category
                    content = "".join(new_lines)
                    if is_surge:
                        # 插入在 #!name 之后或文件头部
                        if "#!name" in content:
                            content = re.sub(r'^(#!name.*)', f'\\1\n#!category={cat_string}', content, flags=re.MULTILINE)
                        else:
                            content = f"#!category={cat_string}\n" + content
                    else:
                        # Shadowrocket 风格
                        content = f"# category: {cat_string}\n" + content
                    
                    with open(file_path, "w", encoding="utf-8") as f:
                        f.write(content)
                    print(f"Fixed (cleaned duplicates) for: {file_path}")

if __name__ == "__main__":
    fix_categories()
