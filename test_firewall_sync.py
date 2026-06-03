module_lines = ["[General]\n", "a=1\n", "\n", "[Host]\n", "b=2\n", "\n", "[Rule]\n", "c=3\n"]
START_MARKER = "# START"
END_MARKER = "# END"
new_module_content = []

# first loop:
module_content_str = "".join(module_lines)
if START_MARKER not in module_content_str:
    rule_found = False
    in_rule_section = False
    for line in module_lines:
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_rule_section = (stripped.lower() == "[rule]")
        
        if stripped.lower() == "[rule]":
            rule_found = True
            new_module_content.append(line)
            new_module_content.append(f"\n{START_MARKER}\n")
            new_module_content.append(f"{END_MARKER}\n")
            continue
            
        new_module_content.append(line)
    
    module_lines = new_module_content
    new_module_content = []

print("After first loop module_lines:")
print("".join(module_lines))

# second loop:
skip = False
in_rule_section = False
for line in module_lines:
    stripped = line.strip()
    if stripped.startswith("[") and stripped.endswith("]"):
        in_rule_section = (stripped.lower() == "[rule]")
    if START_MARKER in line:
        new_module_content.append(line)
        skip = True
        continue
    if END_MARKER in line:
        skip = False
        new_module_content.append(line)
        continue
    if not skip:
        new_module_content.append(line)

print("After second loop:")
print("".join(new_module_content))
