use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

fn main() {
    let pump = Pubkey::from_str("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P").unwrap();
    for c in &[
        "6nU2L7MQVUWjtdKHVpuZA9aind73nd3rXC4YFo8KQCy4",  // bot read
        "F67NbSzshYUpAUTFWFEfYgpKjSdkPebV3RXHcMMejDqi",  // inspector read
    ] {
        let creator = Pubkey::from_str(c).unwrap();
        let (cv, _) = Pubkey::find_program_address(&[b"creator-vault", creator.as_ref()], &pump);
        println!("creator={} -> vault={}", c, cv);
    }
}
