
import os
import re

# Domain list from Mihon
mihon_domains = [
    "e-hentai.org", "hitomi.la", "nhentai.net", "pururin.me", "hentai20.io", "hentai2read.com",
    "ahottie.top", "akuma.moe", "allporncomics.co", "asmhentai.com", "baobua.net", "3600000.xyz",
    "buondua.com", "comick.live", "comicsvalley.com", "coomer.st", "cosplaytele.com", "danbooru.donmai.us",
    "elitebabes.com", "sexarthub.com", "femangels.com", "centerfoldhunter.com", "rylskyhunter.com",
    "penthousehub.com", "zemanihunter.com", "metheaven.com", "metxhunter.com", "eroticandbeauty.com",
    "alshunter.com", "digitalsweeties.com", "erroticahunter.com", "mplhunter.com", "drommhub.com",
    "w4bhub.com", "domerotica.com", "tlehunter.com", "jperotica.com", "nubileshunter.com",
    "goddesshunter.com", "eroticinmind.com", "eternalbabes.com", "vivhunter.com", "jpbeauties.com",
    "pinuphunter.com", "xarthub.com", "wowsweeties.com", "randallhub.com", "gravurehunter.com",
    "showyhub.com", "stunningsweeties.com", "amourhub.com", "everia.club", "everiaclub.com",
    "femjoyhunter.com", "foamgirl.net", "ftvhunter.com", "globalcomix.com", "hdoujin.org",
    "hennojin.com", "3hentai.net", "hentai-cosplay-xxx.com", "hentaienvy.com", "hentaiera.com",
    "hentaifox.com", "hentaihand.com", "hentairox.com", "hentaizap.com", "honeytoon.com",
    "imhentai.xxx", "joymiihub.com", "meijuntu.com", "kemono.cr", "kiutaku.com", "schale.network",
    "lunaranime.ru", "luscious.net", "manga18.me", "mangaball.net", "mangacrazy.net", "mangadex.org",
    "mangafire.to", "mangaforfree.net", "manhuarmtl.com", "manhwa18.cc", "manhwa18.net",
    "manhwa18uncensored.com", "manhwaclub.net", "manhwa-raw.com", "metarthunter.com", "leemiau.com",
    "mihentai.com", "misskon.com", "mitaku.net", "myreadingmanga.info", "nhentai.com", "nhentai.xxx",
    "niadd.com", "novelcool.com", "panda.chaika.moe", "photos18.com", "pixiv.net", "pmatehunter.com",
    "pornpics.com", "qtoon.com", "rokuhentai.com", "api.cherrymanhwa.com", "seraphic-deviltry.com",
    "simply-cosplay.com", "simply-hentai.com", "tappytoon.com", "global.toomics.com", "twicomi.com",
    "uncensoredmanhwa.us", "xarthunter.com", "xasiat.com", "xgmn8.vip", "xinmeitulu.com",
    "xiutaku.com", "yabai.si", "yaoimangaonline.com", "xchina.co", "arabshentai.com", "arabtoons.net",
    "arbxcomix.com", "webtoonempire-bl.com", "goldenmanga.net", "hentaislayer.net", "mangaxhentai.com",
    "prochan.net", "mangatuk.com", "manhatic.com", "paradise-bl.com", "yonaber.com",
    "yurimoonsub.blogspot.com", "manga.hentai.cat", "evil-manga.eu"
]

target_file = "/Users/nyamiiko/Downloads/GitHub/script_hub/ruleset/Sources/custom/NSFW_manual.txt"

with open(target_file, 'r') as f:
    content = f.read()

# Find existing domains
existing_domains = set(re.findall(r'^DOMAIN-SUFFIX,([^\s,]+)', content, re.MULTILINE))

new_domains = []
for d in mihon_domains:
    if d not in existing_domains:
        new_domains.append(f"DOMAIN-SUFFIX,{d}")
        existing_domains.add(d)

if new_domains:
    with open(target_file, 'a') as f:
        f.write("\n\n# ========== Mihon Sources (Added 2026-04-18) ==========\n")
        f.write("\n".join(new_domains))
        f.write("\n")
    print(f"Added {len(new_domains)} new domains to NSFW_manual.txt")
else:
    print("No new domains to add.")
