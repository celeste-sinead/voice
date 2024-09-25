# Jupyter Notebooks
Install Jupyter:
```sh
sudo apt install jupyter-notebook
```

Install [evcxr_jupyter](https://github.com/evcxr/evcxr/tree/main/evcxr_jupyter):
```sh
rustup component add rust-src
cargo install --locked evcxr_jupyter
evcxr_jupyter --install
```

Run Jupyter, in the root of this repository:
```sh
jupyter notebook
```