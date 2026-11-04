# Examples

Each file is a runnable simpylink model (JSON). Expected output is in `expected_outputs`.

## sum_example.json
3 + 4 = 7

```bash
python backend/run.py &
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/sum_example.json
# {"status":"ok","outputs":{"o":7.0}, ...}
```

## gain_chain.json
Constant 5 -> Gain 2 -> Bias -3 = 7

```bash
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/gain_chain.json
```

## product_divide.json
12 / 3 = 4 (Product with `*/`)

```bash
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/product_divide.json
```

All examples can also be opened in the GUI (File > Open).
