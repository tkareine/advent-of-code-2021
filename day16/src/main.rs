use std::env;
use std::fmt;
use std::fs::File;
use std::io;
use std::ops::Range;

#[derive(Copy, Clone, fmt::Debug)]
struct Hex(u8);

impl From<u8> for Hex {
    fn from(value: u8) -> Self {
        assert!(value < 16);
        Hex(value)
    }
}

#[inline]
fn bits_in_byte() -> usize {
    u8::BITS as usize
}

/// Smart pointer to a buffer of bytes, providing a view to the data
#[derive(fmt::Debug)]
struct ByteBits<'a> {
    bits_start_offset: u8,
    bytes: &'a [u8],
}

impl<'a> ByteBits<'a> {
    fn value_at(&self, range: Range<usize>) -> Option<u64> {
        let range = Range {
            start: range.start + self.bits_start_offset as usize,
            end: range.end + self.bits_start_offset as usize,
        };

        let last_byte_idx = (range.end - 1) / bits_in_byte();

        if last_byte_idx >= self.bytes.len() {
            return None;
        }

        let start_byte_idx = range.start / bits_in_byte();
        let mut curr_byte_idx = start_byte_idx;

        let mut result = 0u64;

        while curr_byte_idx <= last_byte_idx {
            let byte = self.bytes[curr_byte_idx];

            let (bits, bits_len) = {
                // Byte element in the end (which can be the sole element)
                if curr_byte_idx == last_byte_idx {
                    let mask_start_pos = if curr_byte_idx == start_byte_idx {
                        range.start % bits_in_byte()
                    } else {
                        0
                    };
                    let mask_end_pos = match range.end % bits_in_byte() {
                        0 => bits_in_byte(),
                        i => i,
                    };
                    let mask_len = mask_end_pos - mask_start_pos;
                    let bits = if mask_len == bits_in_byte() {
                        byte
                    } else {
                        (byte >> (bits_in_byte() - mask_end_pos)) & ((1u8 << mask_len) - 1)
                    };
                    (bits, mask_len)
                // Byte element at the start
                } else if curr_byte_idx == start_byte_idx {
                    let mask_len = bits_in_byte() - range.start % bits_in_byte();
                    let bits = if mask_len == bits_in_byte() {
                        byte
                    } else {
                        byte & ((1u8 << mask_len) - 1)
                    };
                    (bits, mask_len)
                // Byte element in the middle
                } else {
                    (byte, bits_in_byte())
                }
            };

            result = (result << bits_len) | (bits as u64);

            curr_byte_idx += 1;
        }

        // println!("value_at> result={result} ({result:#b})");

        Some(result)
    }

    fn shift_right(&self, n: usize) -> Option<ByteBits<'a>> {
        let n = (self.bits_start_offset as usize) + n;
        let byte_idx = n / bits_in_byte();
        if byte_idx >= self.bytes.len() {
            None
        } else {
            Some(ByteBits {
                bits_start_offset: (n % bits_in_byte()) as u8,
                bytes: &self.bytes[byte_idx..],
            })
        }
    }
}

impl<'a> From<&'a [u8]> for ByteBits<'a> {
    fn from(value: &'a [u8]) -> Self {
        ByteBits {
            bits_start_offset: 0,
            bytes: value,
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
    fn read(reader: impl io::BufRead) -> Result<Packet, ReadPacketError> {
        let mut bytes = Vec::<u8>::new();
        let mut curr_byte: Option<u8> = None;
        let mut is_even_hex_pos = true;

        for b in reader.bytes() {
            let b = b.map_err(ReadPacketError::ReadFailure)?;

            if b.is_ascii_whitespace() {
                continue;
            }

            let h = byte_to_hex(b)?;

            if is_even_hex_pos {
                curr_byte = Some(h.0 << 4);
            } else {
                bytes.push(curr_byte.unwrap() | h.0);
                curr_byte = None;
            }

            is_even_hex_pos = !is_even_hex_pos;
        }

        if let Some(b) = curr_byte {
            bytes.push(b);
        }

        read_packet(bytes[..].into()).map(|(p, _)| p)
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

fn read_packet(byte_bits: ByteBits) -> Result<(Packet, usize), ReadPacketError> {
    let packet_version = byte_bits
        .value_at(0..3)
        .ok_or(ReadPacketError::IncompleteEncoding)? as u8;

    let packet_type = byte_bits
        .value_at(3..6)
        .ok_or(ReadPacketError::IncompleteEncoding)? as u8;

    let byte_bits = byte_bits
        .shift_right(6)
        .ok_or(ReadPacketError::IncompleteEncoding)?;

    let (packet_payload, payload_len) = if packet_type == LITERAL_PACKET_TYPE_ID {
        read_literal_value(byte_bits).map(|(value, len)| (PacketPayload::Literal { value }, len))
    } else {
        let op_kind = OperatorKind::read(packet_type)
            .unwrap_or_else(|| panic!("Unexpected packet type: {}", packet_type));

        let length_type = byte_bits
            .value_at(0..1)
            .ok_or(ReadPacketError::IncompleteEncoding)?;

        let byte_bits = byte_bits
            .shift_right(1)
            .ok_or(ReadPacketError::IncompleteEncoding)?;

        match length_type {
            0 => read_packets_by_total_len(byte_bits),
            _ => read_packets_by_num_packets(byte_bits),
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

fn read_literal_value(byte_bits: ByteBits) -> Result<(u64, usize), ReadPacketError> {
    let mut has_more = true;
    let mut value = 0u64;
    let mut idx = 0;

    while has_more {
        has_more = byte_bits
            .value_at(idx..idx + 1)
            .ok_or(ReadPacketError::IncompleteEncoding)?
            > 0;

        let v = byte_bits
            .value_at(idx + 1..idx + 5)
            .ok_or(ReadPacketError::IncompleteEncoding)?;

        value = (value << 4) | v;

        idx += 5;
    }

    Ok((value, idx))
}

fn read_packets_by_total_len(byte_bits: ByteBits) -> Result<(Vec<Packet>, usize), ReadPacketError> {
    let total_len = byte_bits
        .value_at(0..15)
        .ok_or(ReadPacketError::IncompleteEncoding)? as usize
        + 15;

    let mut next_packet_idx = 15;

    let mut packets = Vec::<Packet>::new();

    while next_packet_idx < total_len {
        let (p, p_len) = read_packet(
            byte_bits
                .shift_right(next_packet_idx)
                .ok_or(ReadPacketError::IncompleteEncoding)?,
        )?;
        packets.push(p);
        next_packet_idx += p_len;
    }

    Ok((packets, total_len))
}

fn read_packets_by_num_packets(
    byte_bits: ByteBits,
) -> Result<(Vec<Packet>, usize), ReadPacketError> {
    let total_num_packets = byte_bits
        .value_at(0..11)
        .ok_or(ReadPacketError::IncompleteEncoding)? as usize;

    let mut next_packet_idx = 11;

    let mut packets = Vec::<Packet>::new();

    for _ in 0..total_num_packets {
        let (p, p_len) = read_packet(
            byte_bits
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

    let packet = Packet::read(io::BufReader::new(
        File::open(filename).expect("File not found"),
    ))
    .expect("Failed to read packet");

    println!("Packet version sum: {}", packet.sum_versions());
    println!("Packet evaluate: {}", packet.evaluate());
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! byte_bits_test {
        ($name:ident offset: $bits_start_offset:expr , bytes: $bytes:expr , range: $value_range:expr , expected_value: $expected_value:expr) => {
            #[test]
            fn $name() {
                let byte_bits = ByteBits {
                    bits_start_offset: $bits_start_offset,
                    bytes: &$bytes,
                };

                let actual_value = byte_bits.value_at($value_range);

                assert_eq!(actual_value, $expected_value);
            }
        };
    }

    byte_bits_test!(
        byte_bits_offset_0_from_0_to_8
        offset: 0,
        bytes: [0b10110001u8],
        range: 0..8,
        expected_value: Some(0b10110001)
    );

    byte_bits_test!(
        byte_bits_offset_0_from_0_to_5
        offset: 0,
        bytes: [0b11001000u8],
        range: 0..5,
        expected_value: Some(0b11001)
    );

    byte_bits_test!(
        byte_bits_offset_0_from_2_to_7
        offset: 0,
        bytes: [0b00110010u8],
        range: 2..7,
        expected_value: Some(0b11001)
    );

    byte_bits_test!(
        byte_bits_offset_0_from_5_to_8
        offset: 0,
        bytes: [0b10100101u8],
        range: 5..8,
        expected_value: Some(0b101)
    );

    byte_bits_test!(
        byte_bits_offset_0_from_8_to_16
        offset: 0,
        bytes: [0b00000000u8, 0b11100001u8],
        range: 8..16,
        expected_value: Some(0b11100001)
    );

    byte_bits_test!(
        byte_bits_offset_0_from_6_to_12
        offset: 0,
        bytes: [0b00000010u8, 0b01010000u8],
        range: 6..12,
        expected_value: Some(0b100101)
    );

    byte_bits_test!(
        byte_bits_offset_0_from_0_to_9
        offset: 0,
        bytes: [0b10110001u8, 0b10000001u8],
        range: 0..9,
        expected_value: Some(0b101100011)
    );

    byte_bits_test!(
        byte_bits_offset_1_from_2_to_8
        offset: 1,
        bytes: [0b11011001u8, 0b01110000u8],
        range: 2..8,
        expected_value: Some(0b110010)
    );

    byte_bits_test!(
        byte_bits_offset_0_from_6_to_7
        offset: 0,
        bytes: [0b00000010u8, 0b00000000u8],
        range: 6..7,
        expected_value: Some(0b1)
    );

    byte_bits_test!(
        byte_bits_offset_2_from_6_to_7
        offset: 2,
        bytes: [0b00000000u8, 0b10000000u8],
        range: 6..7,
        expected_value: Some(0b1)
    );

    #[test]
    fn byte_bits_shift_right() {
        let byte_bits: ByteBits = (&[0b00000010u8, 0b10000000u8][..]).into();

        let actual_value = byte_bits
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
