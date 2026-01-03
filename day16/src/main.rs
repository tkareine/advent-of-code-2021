use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead};
use std::ops::Range;

#[derive(Copy, Clone, fmt::Debug)]
struct Hex(u8);

impl From<u8> for Hex {
    fn from(value: u8) -> Self {
        assert!(value < 16);
        Hex(value)
    }
}

/// Smart pointer to a buffer of [`Hex`]es, providing a view to the data
#[derive(fmt::Debug)]
struct HexBits<'a> {
    bits_start_offset: u8,
    hexes: &'a [Hex],
}

impl<'a> HexBits<'a> {
    fn value_at(&self, index: Range<usize>) -> Option<u64> {
        let index = Range {
            start: index.start + self.bits_start_offset as usize,
            end: index.end + self.bits_start_offset as usize,
        };

        let last_hex_idx = (index.end - 1) / 4;

        if last_hex_idx >= self.hexes.len() {
            return None;
        }

        let start_hex_idx = index.start / 4;
        let mut curr_hex_idx = start_hex_idx;

        let mut result = 0u64;

        while curr_hex_idx <= last_hex_idx {
            let hex = self.hexes[curr_hex_idx];

            let (bits, bits_len) = if curr_hex_idx == last_hex_idx {
                // Hex element in the end
                let mask_start_pos = if curr_hex_idx == start_hex_idx {
                    index.start % 4
                } else {
                    0
                };
                let mask_end_pos = match index.end % 4 {
                    0 => 4,
                    i => i,
                };
                let mask_len = mask_end_pos - mask_start_pos;
                let bits = (hex.0 >> (4 - mask_end_pos)) & ((1u8 << mask_len) - 1);
                (bits, mask_len)
            } else if curr_hex_idx == start_hex_idx {
                // Hex element at the start
                let mask_len = 4 - index.start % 4;
                let bits = hex.0 & ((1u8 << mask_len) - 1);
                (bits, mask_len)
            } else {
                // Hex element in the middle
                let mask_len = 4;
                let bits = hex.0 & ((1u8 << 4) - 1);
                (bits, mask_len)
            };

            result = (result << bits_len) | (bits as u64);

            curr_hex_idx += 1;
        }

        // println!("value_at> result={result} ({result:#b})");

        Some(result)
    }

    fn shift_right(&self, n: usize) -> Option<HexBits<'a>> {
        let n = (self.bits_start_offset as usize) + n;
        let hex_idx = n / 4;
        if hex_idx >= self.hexes.len() {
            None
        } else {
            Some(HexBits {
                bits_start_offset: (n % 4) as u8,
                hexes: &self.hexes[hex_idx..],
            })
        }
    }
}

impl<'a> From<&'a [Hex]> for HexBits<'a> {
    fn from(value: &'a [Hex]) -> Self {
        HexBits {
            bits_start_offset: 0,
            hexes: value,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
enum ReadPacketError {
    ReadFailure(io::Error),
    IncompleteEncoding,
    InvalidEncoding,
}

const LITERAL_PACKET_TYPE_ID: u8 = 4;

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
enum PacketPayload {
    Literal {
        value: u64,
    },
    Operator {
        kind: OperatorKind,
        packets: Vec<Packet>,
    },
}

#[derive(Debug, PartialEq)]
enum OperatorKind {
    Sum,
    Prod,
    Min,
    Max,
    Gt,
    Lt,
    Eq,
}

impl OperatorKind {
    fn read(packet_type: u8) -> Option<OperatorKind> {
        use OperatorKind::*;

        match packet_type {
            0 => Some(Sum),
            1 => Some(Prod),
            2 => Some(Min),
            3 => Some(Max),
            5 => Some(Gt),
            6 => Some(Lt),
            7 => Some(Eq),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq)]
struct Packet {
    version: u8,
    payload: PacketPayload,
}

fn byte_to_hex(b: u8) -> Result<Hex, ReadPacketError> {
    if b.is_ascii_digit() {
        Ok((b - b'0').into())
    } else if b.is_ascii_hexdigit() {
        Ok((b.to_ascii_lowercase() - b'a' + 10).into())
    } else {
        Err(ReadPacketError::InvalidEncoding)
    }
}

impl Packet {
    fn read(reader: impl BufRead) -> Result<Packet, ReadPacketError> {
        let bytes = reader
            .bytes()
            .map(|b| b.map_err(ReadPacketError::ReadFailure))
            .collect::<Result<Vec<u8>, ReadPacketError>>()?;

        let hexes = bytes
            .into_iter()
            .filter_map(|b| {
                if b.is_ascii_whitespace() {
                    None
                } else {
                    Some(byte_to_hex(b))
                }
            })
            .collect::<Result<Vec<Hex>, ReadPacketError>>()?;

        read_packet(hexes[..].into()).map(|(p, _)| p)
    }

    fn sum_versions(&self) -> u64 {
        let v = self.version as u64;
        let ss = match &self.payload {
            PacketPayload::Operator { kind: _, packets } => {
                packets.iter().map(|p| p.sum_versions()).sum()
            }
            _ => 0,
        };
        v + ss
    }

    fn evaluate(&self) -> u64 {
        use OperatorKind::*;
        use PacketPayload::*;

        match &self.payload {
            Literal { value } => *value,
            Operator { kind, packets } => match kind {
                Sum => packets.iter().map(|p| p.evaluate()).sum(),
                Prod => packets.iter().map(|p| p.evaluate()).product(),
                Min => packets.iter().map(|p| p.evaluate()).min().unwrap_or(0),
                Max => packets.iter().map(|p| p.evaluate()).max().unwrap_or(0),
                Gt => {
                    if packets[0].evaluate() > packets[1].evaluate() {
                        1
                    } else {
                        0
                    }
                }
                Lt => {
                    if packets[0].evaluate() < packets[1].evaluate() {
                        1
                    } else {
                        0
                    }
                }
                Eq => {
                    if packets[0].evaluate() == packets[1].evaluate() {
                        1
                    } else {
                        0
                    }
                }
            },
        }
    }
}

fn read_packet(hex_bits: HexBits) -> Result<(Packet, usize), ReadPacketError> {
    let packet_version = hex_bits
        .value_at(0..3)
        .ok_or(ReadPacketError::IncompleteEncoding)? as u8;

    let packet_type = hex_bits
        .value_at(3..6)
        .ok_or(ReadPacketError::IncompleteEncoding)? as u8;

    let hex_bits = hex_bits
        .shift_right(6)
        .ok_or(ReadPacketError::IncompleteEncoding)?;

    let (packet_payload, payload_len) = if packet_type == LITERAL_PACKET_TYPE_ID {
        read_literal_value(hex_bits).map(|(value, len)| (PacketPayload::Literal { value }, len))
    } else {
        let op_kind = OperatorKind::read(packet_type)
            .unwrap_or_else(|| panic!("Unexpected packet type: {}", packet_type));

        let length_type = hex_bits
            .value_at(0..1)
            .ok_or(ReadPacketError::IncompleteEncoding)?;

        let hex_bits = hex_bits
            .shift_right(1)
            .ok_or(ReadPacketError::IncompleteEncoding)?;

        match length_type {
            0 => read_packets_by_total_len(hex_bits),
            _ => read_packets_by_num_packets(hex_bits),
        }
        .map(|(packets, len)| {
            let pp = PacketPayload::Operator {
                kind: op_kind,
                packets,
            };
            (pp, len + 1)
        })
    }?;

    let packet = Packet {
        version: packet_version,
        payload: packet_payload,
    };

    Ok((packet, 6 + payload_len))
}

fn read_literal_value(hex_bits: HexBits) -> Result<(u64, usize), ReadPacketError> {
    let mut has_more = true;
    let mut value = 0u64;
    let mut idx = 0;

    while has_more {
        has_more = hex_bits
            .value_at(idx..idx + 1)
            .ok_or(ReadPacketError::IncompleteEncoding)?
            > 0;

        let v = hex_bits
            .value_at(idx + 1..idx + 5)
            .ok_or(ReadPacketError::IncompleteEncoding)?;

        value = (value << 4) | v;

        idx += 5;
    }

    Ok((value, idx))
}

fn read_packets_by_total_len(hex_bits: HexBits) -> Result<(Vec<Packet>, usize), ReadPacketError> {
    let total_len = hex_bits
        .value_at(0..15)
        .ok_or(ReadPacketError::IncompleteEncoding)? as usize
        + 15;

    let mut next_packet_idx = 15;

    let mut packets = Vec::<Packet>::new();

    while next_packet_idx < total_len {
        let (p, p_len) = read_packet(
            hex_bits
                .shift_right(next_packet_idx)
                .ok_or(ReadPacketError::IncompleteEncoding)?,
        )?;
        packets.push(p);
        next_packet_idx += p_len;
    }

    Ok((packets, total_len))
}

fn read_packets_by_num_packets(hex_bits: HexBits) -> Result<(Vec<Packet>, usize), ReadPacketError> {
    let total_num_packets = hex_bits
        .value_at(0..11)
        .ok_or(ReadPacketError::IncompleteEncoding)? as usize;

    let mut next_packet_idx = 11;

    let mut packets = Vec::<Packet>::new();

    for _ in 0..total_num_packets {
        let (p, p_len) = read_packet(
            hex_bits
                .shift_right(next_packet_idx)
                .ok_or(ReadPacketError::IncompleteEncoding)?,
        )?;
        packets.push(p);
        next_packet_idx += p_len;
    }

    Ok((packets, next_packet_idx))
}

/// CLI usage: cargo run --release -- input.txt
fn main() {
    let filename = env::args().nth(1).expect("Missing input file");

    let packet = Packet::read(&mut io::BufReader::new(
        File::open(filename).expect("File not found"),
    ))
    .expect("Failed to read packet");

    println!("Packet version sum: {}", packet.sum_versions());
    println!("Packet evaluate: {}", packet.evaluate());
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! hex_bits_test {
        ($name:ident offset: $bits_start_offset:expr , hexes: $hexes:expr , index: $value_index:expr , expect_value: $expected_value:expr) => {
            #[test]
            fn $name() {
                let hs: Vec<Hex> = $hexes.into_iter().map(|h| h.into()).collect();

                let hex_bits = HexBits {
                    bits_start_offset: $bits_start_offset,
                    hexes: &hs,
                };

                let actual_value = hex_bits.value_at($value_index);

                assert_eq!(actual_value, $expected_value);
            }
        };
    }

    hex_bits_test!(
        hex_bits_offset_0_from_0_to_4
        offset: 0,
        hexes: vec![0b1011u8],
        index: 0..4,
        expect_value: Some(0b1011)
    );

    hex_bits_test!(
        hex_bits_offset_0_from_0_to_3
        offset: 0,
        hexes: vec![0b1101u8],
        index: 0..3,
        expect_value: Some(0b110)
    );

    hex_bits_test!(
        hex_bits_offset_0_from_1_to_3
        offset: 0,
        hexes: vec![0b1101u8],
        index: 1..3,
        expect_value: Some(0b10)
    );

    hex_bits_test!(
        hex_bits_offset_0_from_1_to_4
        offset: 0,
        hexes: vec![0b1101u8],
        index: 1..4,
        expect_value: Some(0b101)
    );

    hex_bits_test!(
        hex_bits_offset_0_from_2_to_6
        offset: 0,
        hexes: vec![0b1110u8, 0b1011u8],
        index: 2..6,
        expect_value: Some(0b1010)
    );

    hex_bits_test!(
        hex_bits_offset_0_from_3_to_9
        offset: 0,
        hexes: vec![0b1101u8, 0b1001u8, 0b0111u8],
        index: 3..9,
        expect_value: Some(0b110010)
    );

    hex_bits_test!(
        hex_bits_offset_1_from_2_to_8
        offset: 1,
        hexes: vec![0b1101u8, 0b1001u8, 0b0111u8],
        index: 2..8,
        expect_value: Some(0b110010)
    );

    hex_bits_test!(
        hex_bits_offset_0_from_10_11
        offset: 0,
        hexes: vec![0b0000u8, 0b0000u8, 0b0010u8, 0b0000u8],
        index: 10..11,
        expect_value: Some(0b1)
    );

    hex_bits_test!(
        hex_bits_offset_2_from_10_11
        offset: 2,
        hexes: vec![0b0000u8, 0b0000u8, 0b0000u8, 0b1000u8],
        index: 10..11,
        expect_value: Some(0b1)
    );

    #[test]
    fn hex_bits_shift_right() {
        let hs: Vec<Hex> = vec![0b0000u8, 0b0010u8, 0b1000u8]
            .into_iter()
            .map(|h| h.into())
            .collect();
        let hex_bits: HexBits = (&hs[..]).into();

        let actual_value = hex_bits
            .shift_right(3)
            .unwrap()
            .shift_right(2)
            .unwrap()
            .value_at(1..4)
            .unwrap();

        assert_eq!(actual_value, 0b101u64);
    }

    #[test]
    fn read_literal_packet() {
        assert_eq!(
            read_packet("D2FE28"),
            Packet {
                version: 6,
                payload: PacketPayload::Literal { value: 2021 }
            }
        );
    }

    #[test]
    fn read_operator_packet_by_total_len() {
        assert_eq!(
            read_packet("38006F45291200"),
            Packet {
                version: 1,
                payload: PacketPayload::Operator {
                    kind: OperatorKind::Lt,
                    packets: vec![
                        Packet {
                            version: 6,
                            payload: PacketPayload::Literal { value: 10 }
                        },
                        Packet {
                            version: 2,
                            payload: PacketPayload::Literal { value: 20 }
                        }
                    ]
                }
            }
        );
    }

    #[test]
    fn read_operator_packet_by_num_packets() {
        assert_eq!(
            read_packet("EE00D40C823060"),
            Packet {
                version: 7,
                payload: PacketPayload::Operator {
                    kind: OperatorKind::Max,
                    packets: vec![
                        Packet {
                            version: 2,
                            payload: PacketPayload::Literal { value: 1 }
                        },
                        Packet {
                            version: 4,
                            payload: PacketPayload::Literal { value: 2 }
                        },
                        Packet {
                            version: 1,
                            payload: PacketPayload::Literal { value: 3 }
                        }
                    ]
                }
            }
        );
    }

    macro_rules! packet_sum_versions_test {
        ($name:ident encoding: $encoding:expr , expected_sum: $expected_sum:expr) => {
            #[test]
            fn $name() {
                let packet = read_packet($encoding);
                assert_eq!(packet.sum_versions(), $expected_sum);
            }
        };
    }

    packet_sum_versions_test!(
        packet_sum_versions_example_1
        encoding: "8A004A801A8002F478",
        expected_sum: 16
    );

    packet_sum_versions_test!(
        packet_sum_versions_example_2
        encoding: "620080001611562C8802118E34",
        expected_sum: 12
    );

    packet_sum_versions_test!(
        packet_sum_versions_example_3
        encoding: "C0015000016115A2E0802F182340",
        expected_sum: 23
    );

    packet_sum_versions_test!(
        packet_sum_versions_example_4
        encoding: "A0016C880162017C3686B18A3D4780",
        expected_sum: 31
    );

    macro_rules! packet_evaluate_test {
        ($name:ident encoding: $encoding:expr , expected_value: $expected_value:expr) => {
            #[test]
            fn $name() {
                let packet = read_packet($encoding);
                assert_eq!(packet.evaluate(), $expected_value);
            }
        };
    }

    packet_evaluate_test!(
        packet_evaluate_sum
        encoding: "C200B40A82",
        expected_value: 3
    );

    packet_evaluate_test!(
        packet_evaluate_prod
        encoding: "04005AC33890",
        expected_value: 54
    );

    packet_evaluate_test!(
        packet_evaluate_min
        encoding: "880086C3E88112",
        expected_value: 7
    );

    packet_evaluate_test!(
        packet_evaluate_max
        encoding: "CE00C43D881120",
        expected_value: 9
    );

    packet_evaluate_test!(
        packet_evaluate_lt
        encoding: "D8005AC2A8F0",
        expected_value: 1
    );

    packet_evaluate_test!(
        packet_evaluate_gt
        encoding: "F600BC2D8F",
        expected_value: 0
    );

    packet_evaluate_test!(
        packet_evaluate_eq
        encoding: "9C005AC2F8F0",
        expected_value: 0
    );

    packet_evaluate_test!(
        packet_evaluate_sum_eq_prod
        encoding: "9C0141080250320F1802104A08",
        expected_value: 1
    );

    fn read_packet(s: &str) -> Packet {
        Packet::read(io::BufReader::new(s.as_bytes())).expect("Failed to read packet")
    }
}
