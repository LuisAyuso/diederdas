#!/usr/bin/env python3
"""Build data/words.tsv: the 2000 most frequent German nouns with one unambiguous gender."""
import csv, io, math, re, sys, tarfile, urllib.request
from pathlib import Path

NOUNS = "https://raw.githubusercontent.com/gambolputty/german-nouns/main/german_nouns/nouns.csv"
SUBS = "https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/de/de_50k.txt"
NEWS = "https://downloads.wortschatz-leipzig.de/corpora/deu_news_2023_300K.tar.gz"
COUNT = 2000
# Proper names, English words, plurals and inflected forms that slip through the filters.
EXCLUDE = set("""
Nein Namen China Hallo Ach City Anna Fällen Willen Sir Miss Klaus Rom Schwarz Müller Hans
West Real Henry Benjamin Kate King League Schiffe Marie Opfern El Media Date Lady Top Schneider
Charlotte Kai Madame Queen Green Bob My Luke Bach Matt Wagner Scheiß Iris Baden Oscar Junior
Bobby Nord No Formen Noah Funk Re Fritz Au Tommy Reihen Rande Football First Drink Power
Wall Dirk Jean College Steven Pop Burger Mac Amazon German Business Hermann Penny Gelb Manchester Axel
Horst Mister Walker Dax Weber Wetten Vögeln Kiel Pascal Barbie Royal Pariser Berliner Hamburger Alpha Grant
Go Hugo Microsoft End Ken Khan Hau Marina Are Kreisen Zwecke Hexen Master Turner Gedanken
Blicke Londoner Esther Gloria Boxen Cop Cloud Sound Dinner Derby Tesla Hagen
""".split())
ARTICLE = {"m": "der", "f": "die", "n": "das"}
ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "tools" / ".cache"


def fetch(url):
    path = CACHE / url.rsplit("/", 1)[1]
    if not path.exists():
        CACHE.mkdir(exist_ok=True)
        print("download", url, file=sys.stderr)
        urllib.request.urlretrieve(url, path)
    return path


def genders():
    seen = {}
    with open(fetch(NOUNS), encoding="utf-8") as f:
        for row in csv.DictReader(f):
            if row["pos"] != "Substantiv":
                continue
            g = {row[k] for k in ("genus", "genus 1", "genus 2", "genus 3", "genus 4") if row.get(k)}
            seen.setdefault(row["lemma"], set()).update(g)
    word = re.compile(r"^[A-ZÄÖÜ][a-zäöüß]+$")
    return {w: g.pop() for w, g in seen.items() if len(g) == 1 and word.match(w)}


def ranks_subtitles():
    with open(fetch(SUBS), encoding="utf-8") as f:
        return {line.split()[0]: i for i, line in enumerate(f)}


def counts_news():
    counts = {}
    with tarfile.open(fetch(NEWS)) as tar:
        member = next(m for m in tar.getmembers() if m.name.endswith("-words.txt"))
        for line in io.TextIOWrapper(tar.extractfile(member), encoding="utf-8"):
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 3:
                counts[parts[1]] = counts.get(parts[1], 0) + int(parts[2])
    return counts


def main():
    nouns = genders()
    subs, counts = ranks_subtitles(), counts_news()
    news = {w: i for i, w in enumerate(sorted(counts, key=counts.get, reverse=True))}
    # Drop nominalised function words and verbs (das Ich, das Wie, das Sein): the lowercase form dominates.
    nouns = {w: g for w, g in nouns.items() if counts.get(w, 0) >= 2 * counts.get(w.lower(), 0) and w not in EXCLUDE}
    missing = 200_000
    # Geometric mean of both ranks: a word must be common in speech (subtitles) and in writing (news).
    score = {w: math.sqrt(subs.get(w.lower(), missing) * news.get(w, missing)) for w in nouns}
    top = sorted(score, key=score.get)[:COUNT]
    out = ROOT / "data" / "words.tsv"
    out.write_text("".join(f"{ARTICLE[nouns[w]]}\t{w}\n" for w in top), encoding="utf-8")
    print(f"wrote {len(top)} nouns to {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
