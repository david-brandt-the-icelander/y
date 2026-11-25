# The `site` step of y's `pages` pipeline: what .github/workflows/pages.yml's build job does, in the image of the action
# it uses (actions/jekyll-build-pages v1 is this tag: the github-pages gem 232, Jekyll 3.10). The image exists for
# linux/amd64 only, so the pipeline runs this step with that platform. python3 is added for .github/pages/stage.py.
FROM ghcr.io/actions/jekyll-build-pages:v1.0.13
RUN apt-get update && apt-get install -y --no-install-recommends python3 && rm -rf /var/lib/apt/lists/*
COPY run.sh /run.sh
ENTRYPOINT []
CMD ["bash", "/run.sh"]
