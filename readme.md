# Godwoken Stress Test

## Setup

- ckb and ckb-cli
- jq
- gw-tools

## create accounts and deposit ckb

### import_ckb_account.sh $CKB_URL $GW_URL

./import_ckb_account.sh http://localhost:8114 http://localhost:8119

## run

```shell
gwst -h
RUST_LOG=info gwst run -b 3 -i 100 -t 120 -s scripts-deploy-result.json -u http://$(GODWOKEN_RPC_URL):8119 -h $ROLLUP_TYPE_HASH
```