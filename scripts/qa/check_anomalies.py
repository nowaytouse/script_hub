
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RULESETS_DIR = ROOT / "rulesets"
MODULES_DIR = ROOT / "modules"

def check():
    errors = []
    
    # 1. Check for DOMAIN-REGEX in Surge list files
    surge_lists = list(RULESETS_DIR.glob("**/*.list"))
    for file in surge_lists:
        content = file.read_text(encoding='utf-8')
        lines = content.split('\n')
        
        # Check empty files
        if not lines or all(not l.strip() for l in lines):
            errors.append(f"Empty file: {file}")
            
        for i, line in enumerate(lines):
            line = line.strip()
            if not line or line.startswith('#') or line.startswith('//'):
                continue
            
            # Check for DOMAIN-REGEX
            if line.startswith("DOMAIN-REGEX"):
                errors.append(f"{file.name}:{i+1} Contains forbidden DOMAIN-REGEX: {line}")
                
            # Check for unstripped comments in payload
            if ("#" in line and not line.startswith('URL-REGEX') and not line.startswith('DOMAIN-REGEX')) or \
               (line.startswith('PROCESS-NAME') and '#' in line):
                # actually URL-REGEX can have # but it's rare
                if "PROCESS-NAME" in line and '#' in line:
                    errors.append(f"{file.name}:{i+1} Unstripped comment in rule: {line}")

            if "URL-REGEX,https:" in line and "://" not in line:
                errors.append(f"{file.name}:{i+1} Invalid URL-REGEX: {line}")

            if "PROCESS-NAME" in line and " " in line.split(",")[1]:
                if "PROCESS-NAME" in line and "#" in line:
                     pass # already caught
            
            if ",REJECT" in line and "URL-REGEX" in line:
                if len(line.split(",")) > 2:
                    # check if the last part is REJECT but it was stripped
                    pass

    for error in errors:
        print(error)
        
    print(f"Total anomalies found: {len(errors)}")

if __name__ == '__main__':
    check()
