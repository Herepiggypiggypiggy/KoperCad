# novoelectric (Python API)

Open, edit and save NovoElectric project files from Python, headless. Every
call is one command and one undo step, the same commands the app runs.

## Build and install (no admin rights)

```
pip install --user maturin
cd crates/novo-py
maturin build --release            # wheel in target/wheels (abi3: Python 3.9+)
pip install --user <the wheel>
```

`pip install --user` writes only under your profile (`%APPDATA%\Python` on
Windows). In a virtual environment, `maturin develop --release` builds and
installs in one step. Without Python, `cargo build`/`cargo test` skip the
bindings: they live behind the `python` feature, which maturin turns on.

## Example

```python
import novoelectric as ne

with ne.Project.create("dol.novo", name="DOL starter") as p:
    page = p.add_page("1", "Main circuit")
    l1 = p.add_potential("L1", "line", 400.0)
    k1 = p.add_device("-K1", "Motor contactor")
    feed = p.place(page, "feed", 40, 0, potential=l1)        # alias or library id
    main = p.place(page, "km3", 40, 40, device=k1)
    p.connect((feed, "1"), (main, "1"))
    p.set_tag(k1, "=A1+CAB-K1")                               # rename
    p.undo(); p.redo()
    p.save()
    print(p.nets())     # [{"pins": ["-K1:1", ...], "potentials": ["L1"], ...}]
```

- Queries: `info()`, `pages()`, `devices()`, `symbols()`, `wires()`,
  `potentials()`, `terminals()`, `nets()`, `device(tag)`, `to_json()`;
  `novoelectric.symbols()` lists the symbol library.
- Any model command in its JSON form: `p.execute({"command": "add_page",
  "name": "3"})`; a `{"command": "batch", ...}` is one undo step.
- Errors raise `novoelectric.NovoError`; a refused call changes nothing.
- A file stays locked against the app while it is open in a script. A file
  with unrecovered work from a crashed app session is refused, never saved
  over.

## Tests

```
python -m pytest crates/novo-py/python/tests
```
