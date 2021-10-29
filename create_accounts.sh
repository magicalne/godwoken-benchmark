amount=1000
count=10

base='accounts/privkey_'
for i in $(seq $count); do
    pk_path=$base$i
    account_path=$pk_path$suffix
    if [ ! -f $account_path ]; then
            openssl rand -hex 32 > $pk_path
    fi
    read pk < $pk_path

	curl "$1/deposit?eth_address=0x$pk" \
  -H 'Connection: keep-alive' \
  -H 'Accept: application/json, text/plain, */*' \
  --compressed

done
