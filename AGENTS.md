# Agent instructions

This is the repository of the Y programming language. So far it holds only `README.md` and `LICENSE` (Mozilla Public License 2.0): there is no source code, build or test suite yet.

## Architecture diagrams and knowledge files

Each folder documents itself in a `.dev/` directory next to its code, as JSON following the skills in `Meta/Agents/Skills/` and the knowledge catalogue in `Meta/Agents/Knowledge/` (human-assisted software development). Read a folder's `.dev` files before changing it. `populate-dev` decides what a folder needs.

Today only the repository root is documented: a `folder` record in `.dev/folder.json`, and a component diagram in `.dev/component-diagram.json` whose one part is `Meta/Agents`. No folder holds code to draw yet. As code arrives, the diagrams are:

- `describe-component-diagram`: the folder is a cohesive capability, adapter, boundary or subsystem (the repository root, and each such folder of the language's source); the root's component diagram lists its direct children and links to their diagrams.
- `describe-class-diagram`: the folder itself declares more than one class.
- `describe-state-diagram`: the folder implements a state machine. One `<Identifier>.state-diagram.json` per machine, named after the class or type it belongs to.
- `describe-deployment-diagram`: the folder holds something independently deployable or published on its own.

What a folder does, offers and exposes goes in its use cases, user journeys, capabilities, features and operations (`describe-use-case`, `describe-user-journey`, `describe-capability`, `describe-feature`, `describe-operation`). What it intends, promises and needs (requirements, backlog items, decisions, blockers, contracts, verification, delivery, configuration, runbooks, limitations and the rest) goes in knowledge files, written with `describe-knowledge`. Wanted work is a `backlog` item in the folder that will own it, and a question only the owner can answer is a `decisions` record with `status: open`.

Every actor a use case, journey or diagram names has a file in `Actors/` at the repository root, written with `describe-actor` and linked from whatever names it; delete an actor nothing links to. No `.dev` file names an actor yet, so there is no `Actors/` folder. Update the `.dev` files and actors in the same commit as the code they describe, and delete one that stops applying. Before finishing, run both validators from the repository root, with the `jsonschema` package installed:

```bash
python3 Meta/Agents/Skills/validate_diagrams.py
python3 Meta/Agents/Knowledge/validate_knowledge.py
```

The skills and the catalogue come from <https://github.com/david-brandt-the-icelander/human-assisted-software-development> (`Agents/Skills` and `Agents/Knowledge`). Change them there and copy them here, so there is one version.
