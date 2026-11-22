"""Stage the documentation for GitHub Pages.

Copies documentation/ and Y-current-language-design.md into the output folder,
keeping their relative layout so links between them keep working. A link to
something that is not staged (tools/, examples/, proposal/, the licence) is
rewritten to the same path on GitHub, so it does not 404 on the site.

usage: python3 .github/pages/stage.py <repo-root> <output-dir> <github-blob-base>
"""
import os, re, shutil, sys

root, out, base = sys.argv[1], sys.argv[2], sys.argv[3].rstrip("/")
shutil.rmtree(out, ignore_errors=True)
shutil.copytree(os.path.join(root, "documentation"), os.path.join(out, "documentation"))
shutil.copy(os.path.join(root, "Y-current-language-design.md"), out)
shutil.copy(os.path.join(root, ".github", "pages", "_config.yml"), out)
shutil.copy(os.path.join(root, ".github", "pages", "index.md"), out)

LINK = re.compile(r"(\]\()([^)\s]+)(\))")
for dirpath, _, files in os.walk(out):
    for name in files:
        if not name.endswith(".md"):
            continue
        path = os.path.join(dirpath, name)
        rel_dir = os.path.relpath(dirpath, out)
        text = open(path, encoding="utf8").read()

        def fix(m):
            target = m.group(2)
            if re.match(r"(https?:|mailto:|#)", target):
                return m.group(0)
            file_part, hash_, frag = target.partition("#")
            if not file_part:
                return m.group(0)
            staged = os.path.normpath(os.path.join(dirpath, file_part))
            if os.path.exists(staged):
                return m.group(0)
            repo_rel = os.path.normpath(os.path.join(rel_dir, file_part))
            if repo_rel.startswith(".."):
                return m.group(0)
            if not os.path.exists(os.path.join(root, repo_rel)):
                return m.group(0)
            return m.group(1) + base + "/" + repo_rel + hash_ + frag + m.group(3)

        new = LINK.sub(fix, text)
        if new != text:
            open(path, "w", encoding="utf8").write(new)
