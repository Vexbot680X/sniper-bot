use solana_sdk::pubkey::Pubkey;

fn main() {
    // bytes from offset 49 of BC for mint 2GBSJtdZszZXMXfbjRpRc4PcPhbHqK3qaFnu1JqK4PmL
    // hex: d152b9e8b91841e4cb21d494da8c19be1bba7fa42c165c0f7767d611bb197f99
    let pk_bytes: [u8; 32] = [
        0xd1, 0x52, 0xb9, 0xe8, 0xb9, 0x18, 0x41, 0xe4,
        0xcb, 0x21, 0xd4, 0x94, 0xda, 0x8c, 0x19, 0xbe,
        0x1b, 0xba, 0x7f, 0xa4, 0x2c, 0x16, 0x5c, 0x0f,
        0x77, 0x67, 0xd6, 0x11, 0xbb, 0x19, 0x7f, 0x99,
    ];
    let pk = Pubkey::new_from_array(pk_bytes);
    println!("offset 49 as Pubkey: {}", pk);
    println!("Expected per inspector: F67NbSzshYUpAUTFWFEfYgpKjSdkPebV3RXHcMMejDqi");
    println!("Bot logged at runtime:  6nU2L7MQVUWjtdKHVpuZA9aind73nd3rXC4YFo8KQCy4");
}
