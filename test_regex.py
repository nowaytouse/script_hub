import re

text = """[General]
a=1

[Host]

[Rule]
c=3
"""
pattern = re.compile(r"(?ms)^\[Host\]\n.*?(?=^\[[^\n]+\]\n|\Z)")
match = pattern.search(text)
print("Match found:", match is not None)
