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
  -H 'sec-ch-ua: "Google Chrome";v="95", "Chromium";v="95", ";Not A Brand";v="99"' \
  -H 'Accept: application/json, text/plain, */*' \
  -H 'sec-ch-ua-mobile: ?0' \
  -H 'User-Agent: Mozilla/5.0 (X11; Fedora; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/95.0.4638.54 Safari/537.36' \
  -H 'sec-ch-ua-platform: "Linux"' \
  -H 'Sec-Fetch-Dest: empty' \
  -H 'Accept-Language: en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7,zh-TW;q=0.6' \
  --compressed

done
