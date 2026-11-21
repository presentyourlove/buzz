import codecs

for enc in ['utf-16', 'utf-8', 'cp950', 'gbk']:
    try:
        with codecs.open(r'c:\私人\ai agent\buzz\scratch\thread_all_msgs.txt', 'r', encoding=enc) as f:
            text = f.read()
            if "=================== #" in text:
                print(f"Success with encoding: {enc}")
                sections = text.split("=================== #")
                print(f"Total sections: {len(sections) - 1}")
                for s in sections[1:]:
                    lines = s.strip().splitlines()
                    header = lines[0] if lines else ""
                    body = "\n".join(lines[1:4]) if len(lines) > 1 else ""
                    print(f"#{header}")
                    print(f"   {body[:120]}...\n")
                break
    except Exception as e:
        pass
