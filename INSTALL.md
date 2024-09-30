# Jupyter Notebooks
Install Jupyter Lab:
```sh
pip install jupyterlab
```

Install [evcxr_jupyter](https://github.com/evcxr/evcxr/tree/main/evcxr_jupyter):
```sh
rustup component add rust-src
cargo install --locked evcxr_jupyter # Also evcxr_repl if you want
evcxr_jupyter --install
```

Run Jupyter, in the root of this repository:
```sh
jupyter lab
```