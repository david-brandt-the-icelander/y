# Builds the documentation site as pages.yml's build job does: .github/pages/stage.py stages documentation/ and the
# language design, then the github-pages gem builds it with Jekyll (as the action's entrypoint.sh calls it). Leaves:
#   site.tar.gz   the built site (_site), packed: it is many small files, and `deploy` publishes it whole
#   BUILD         the commit
set -euo pipefail
export HOME=/tmp JEKYLL_ENV=production PAGES_REPO_NWO=david-brandt-the-icelander/y
out=${ZAZZLE_OUTPUT:-/zazzle/output}
src=$(ls -d /zazzle/input/repos/*/y | head -1)
python3 "$src/.github/pages/stage.py" "$src" /tmp/site "https://github.com/david-brandt-the-icelander/y/blob/main"
(cd "$BUNDLE_APP_CONFIG" && github-pages build --source /tmp/site --destination /tmp/_site)
[ -f /tmp/_site/index.html ] || { echo "the site has no index.html" >&2; exit 1; }
tar -czf "$out/site.tar.gz" -C /tmp/_site .
git -c safe.directory='*' -C "$src" rev-parse HEAD > "$out/BUILD"
echo "$(find /tmp/_site -type f | wc -l) files, $(du -h "$out/site.tar.gz" | cut -f1) packed"
echo "y's site built"
