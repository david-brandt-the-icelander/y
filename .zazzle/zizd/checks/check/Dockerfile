# The `check` step of y's `checks` pipeline: the .dev diagrams and knowledge files against their schemas (AGENTS.md).
# Python and the validators' packages are pinned.
FROM python:3.13.9-slim
RUN pip install --no-cache-dir jsonschema==4.26.0 referencing==0.37.0
COPY run.sh /run.sh
CMD ["bash", "/run.sh"]
