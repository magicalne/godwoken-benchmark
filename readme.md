# Godwoken Stress Test

## Setup

- kicker
- ckb and ckb-cli
- jq
- gw-tools

### Build

```shell
cargo install --path .
```

## create accounts and deposit ckb

### import_ckb_account.sh $CKB_URL $GW_URL

```shll
## ./create_account.sh $KICKER_URL
./create_account.sh http://localhost:6100
```

## run

```shell
gwst -h
RUST_LOG=info gwst run -b 3 -i 100 -t 120 -s scripts-deploy-result.json -u http://$(GODWOKEN_RPC_URL):8119 -h $ROLLUP_TYPE_HASH
```