# The `deploy` step of y's `pages` pipeline (deploys: true): publishes the site the `site` step built to the branch
# gh-pages, from /zazzle/input/artifacts/site. It never rebuilds. Needs Y_PAGES_GITHUB_TOKEN.
FROM python:3.13.9-slim
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*
COPY run.sh /run.sh
CMD ["bash", "/run.sh"]
