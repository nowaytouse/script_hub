
import json
import re

def get_domains(url):
    import urllib.request
    try:
        with urllib.request.urlopen(url) as response:
            return json.loads(response.read().decode())
    except Exception as e:
        print(f"Error fetching {url}: {e}")
        return []

def extract_domain(base_url):
    if not base_url: return None
    # Remove protocol
    domain = re.sub(r'^https?://', '', base_url)
    # Remove path and port
    domain = domain.split('/')[0].split(':')[0]
    # Remove 'www.'
    if domain.startswith('www.'):
        domain = domain[4:]
    return domain

# Cursed Repo (All)
cursed_url = "https://raw.githubusercontent.com/yuzono/cursed-manga-repo/repo/index.min.json"
cursed_data = get_domains(cursed_url)
cursed_domains = set()
for item in cursed_data:
    for source in item.get('sources', []):
        d = extract_domain(source.get('baseUrl'))
        if d: cursed_domains.add(d)

# Manga Repo (Selective nsfw: 1)
manga_url = "https://raw.githubusercontent.com/yuzono/manga-repo/repo/index.min.json"
manga_data = get_domains(manga_url)
manga_nsfw_domains = set()
for item in manga_data:
    if item.get('nsfw') == 1:
        for source in item.get('sources', []):
            d = extract_domain(source.get('baseUrl'))
            if d: manga_nsfw_domains.add(d)

all_nsfw = cursed_domains.union(manga_nsfw_domains)
# Sort for cleanliness
sorted_nsfw = sorted(list(all_nsfw))

print("--- START DOMAINS ---")
for d in sorted_nsfw:
    print(f"DOMAIN-SUFFIX,{d}")
print("--- END DOMAINS ---")
