with open(r'c:\私人\ai agent\buzz\scratch\thread_all_msgs.txt', 'r', encoding='utf-8', errors='ignore') as f:
    text = f.read()

sections = text.split("=================== #")
print(f"Total parsed sections: {len(sections) - 1}")
for s in sections[1:]:
    header = s.split("\n")[0]
    content = "\n".join(s.split("\n")[1:6])
    print(f"#{header}")
    print(f"  Snippet: {content[:180]}...")
