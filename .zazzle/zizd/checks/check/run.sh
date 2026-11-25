# Runs both .dev validators from the repository root, as AGENTS.md says to before finishing. Reads the repository in
# place; leaves nothing in /zazzle/output.
set -euo pipefail
export HOME=/tmp PYTHONDONTWRITEBYTECODE=1
cd "$(ls -d /zazzle/input/repos/*/y | head -1)"
python3 Meta/Agents/Skills/validate_diagrams.py
python3 Meta/Agents/Knowledge/validate_knowledge.py
echo "y's .dev files checked"
