# Publishes y's documentation site from the site step's artifacts to the branch gh-pages of
# github.com/david-brandt-the-icelander/y, in sub-steps, checking for a halt request before each (exit 75: a deployment
# that should or must halt stops between sub-steps, never in the middle of one):
#   1. prepare: fetch gh-pages (or start it), and make one commit whose tree is exactly the built site plus .nojekyll
#      (the site is built already; GitHub must not run Jekyll on it again);
#   2. publish: push that commit to gh-pages. Nothing is pushed when the site did not change.
# The site appears at the Pages address only once the repository's Pages source is the branch gh-pages (Settings, Pages;
# today it is GitHub Actions, served by .github/workflows/pages.yml): that switch is part of the cutover.
# Refuses without Y_PAGES_GITHUB_TOKEN (a token that may push to y's contents) or without the site's artifacts. The
# token reaches git through a credential helper from the environment, never on a command line.
#
# DEPLOY_DRY_RUN=1, for trying the step by hand only (no pipeline names it): no token needed, nothing fetched (the
# commit starts gh-pages afresh) and nothing pushed; it shows the commit it would push.
set -euo pipefail
export HOME=/tmp GIT_TERMINAL_PROMPT=0
art=${ZAZZLE_ARTIFACTS:-/zazzle/input/artifacts}/site
dry=${DEPLOY_DRY_RUN:-}
remote=https://github.com/david-brandt-the-icelander/y.git
halted() { if [ -n "${ZAZZLE_HALT_FILE:-}" ] && [ -f "$ZAZZLE_HALT_FILE" ]; then echo "halting before $1: $(cat "$ZAZZLE_HALT_FILE")"; exit 75; fi; }

if [ -z "$dry" ]; then
  [ -n "${Y_PAGES_GITHUB_TOKEN:-}" ] || { echo "refusing: Y_PAGES_GITHUB_TOKEN is needed" >&2; exit 1; }
fi
for f in site.tar.gz BUILD; do
  [ -f "$art/$f" ] || { echo "refusing: the site step's artifacts have no $f ($art)" >&2; exit 1; }
done
commit=$(cat "$art/BUILD")
echo "publishing y's site of $commit to gh-pages${dry:+ (dry run)}"

halted "preparing the commit"
rm -rf /tmp/pages && mkdir /tmp/pages && cd /tmp/pages
git init -q && git config user.name "zizd deployer" && git config user.email "zizd@icelander.net"
git config credential.helper '!f() { echo username=x-access-token; echo "password=$Y_PAGES_GITHUB_TOKEN"; }; f'
parent=""
if [ -z "$dry" ] && git fetch -q --depth 1 "$remote" gh-pages 2>/dev/null; then parent=$(git rev-parse FETCH_HEAD); fi
tar -xzf "$art/site.tar.gz" -C /tmp/pages
touch .nojekyll
git add -A
tree=$(git write-tree)
if [ -n "$parent" ] && [ "$(git rev-parse "$parent^{tree}")" = "$tree" ]; then
  echo "gh-pages already has this site: nothing to publish"; exit 0
fi
new=$(git commit-tree "$tree" ${parent:+-p "$parent"} -m "The site of $commit")
git log --stat --format='%H %s' -1 "$new" | tail -5

halted "publishing"
if [ -n "$dry" ]; then
  echo "dry run: would push $new to $remote gh-pages ($(git ls-tree -r "$new" | wc -l) files)"
else
  git push -q "$remote" "$new:refs/heads/gh-pages"
  echo "y's site published to gh-pages ($new)"
fi
