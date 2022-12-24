scout: FORCE
	cargo b --bin scout --release
	sudo setcap "CAP_NET_RAW+ep" ./target/release/scout

FORCE: