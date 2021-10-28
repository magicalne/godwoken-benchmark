#!/bin/bash
count=1000
amount=1000
base="accounts/privkey_"
# create ckb accouts and transfer ckb
for i in $(seq $count); do
	pk_path=$base$i
	suffix=".json"
	account_path=$pk_path$suffix
	if [ ! -f $account_path ]; then
		openssl rand -hex 32 > $pk_path
		output=$(yes qwe | ckb-cli account import --privkey-path $pk_path --url $1)
		read pk < $pk_path
		echo $output
	
		echo $account_path
		echo '{' >> $account_path
		echo "	\"mainnet\": \"${output:30:46}\"," >> $account_path
		echo "	\"testnet\": \"${output:88:46}\"," >> $account_path
		echo "	\"lock_arg\": \"${output:145}\"," >> $account_path
		echo "	\"privkey\": \"$pk\"" >> $account_path
		echo '}' >> $account_path

	fi
done

cap_str="total: $amount.0 (CKB)"
for i in $(seq $count); do
	pk_path=$base$i
	account_path=$pk_path$suffix
	addr=$(jq -r '.testnet' $account_path)

	ckb-cli wallet transfer --privkey-path issue_pk --to-address $(jq -r '.testnet' $account_path) --capacity $amount --tx-fee 0.00001		
	while true; do
		capacity=$(ckb-cli wallet get-capacity --address $addr --url $1)
		read num1 num2 <<<${capacity//[^0-9]/ }
		if [ $num1 -ge $amount ]; then
			echo "$addr issued $num1"
			break
		fi
	done
	
done

for i in $(seq $count); do
	pk_path=$base$i
	gw-tools deposit-ckb -c 500 -o config.toml -k $pk_path --scripts-deployment-path scripts-deploy-result.json --ckb-rpc $1 -g $2
done
