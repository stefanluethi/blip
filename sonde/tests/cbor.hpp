// Helpers of the tests to read what sonde encoded.

#pragma once

#include <sonde/sonde.hpp>

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace test {

using Bytes = std::vector<std::uint8_t>;

/// The bytes of a hex string, e.g. "a1f6".
inline Bytes hex(const std::string& text) {
    Bytes bytes;
    for (std::size_t i = 0; i + 1 < text.size(); i += 2) {
        bytes.push_back(static_cast<std::uint8_t>(std::stoul(text.substr(i, 2), nullptr, 16)));
    }
    return bytes;
}

/// The items of a CBOR sequence, or nullopt if it does not decode whole.
inline std::optional<std::vector<Bytes>> items(const std::uint8_t* data, std::size_t size) {
    std::vector<Bytes> result;
    const std::uint8_t* const end = data + size;
    while (data < end) {
        CborParser parser;
        CborValue value;
        if (cbor_parser_init(data, static_cast<std::size_t>(end - data), 0, &parser, &value) !=
                CborNoError ||
            cbor_value_advance(&value) != CborNoError) {
            return std::nullopt;
        }
        const std::uint8_t* next = cbor_value_get_next_byte(&value);
        result.emplace_back(data, next);
        data = next;
    }
    return result;
}

inline std::optional<std::vector<Bytes>> items(const Bytes& bytes) {
    return items(bytes.data(), bytes.size());
}

/// The unsigned integer of a Value message, e.g. {"Value": {..., "value": 5}}.
inline std::optional<std::uint64_t> uint_value(const Bytes& message) {
    CborParser parser;
    CborValue root;
    CborValue variant;
    CborValue value;
    std::uint64_t result;
    if (cbor_parser_init(message.data(), message.size(), 0, &parser, &root) != CborNoError ||
        cbor_value_map_find_value(&root, "Value", &variant) != CborNoError ||
        !cbor_value_is_map(&variant) ||
        cbor_value_map_find_value(&variant, "value", &value) != CborNoError ||
        !cbor_value_is_unsigned_integer(&value) ||
        cbor_value_get_uint64(&value, &result) != CborNoError) {
        return std::nullopt;
    }
    return result;
}

/// The bytes of the message of the value 'a': {"Value": {"timestamp": 0, "path": "a",
/// "value": ...}} without the value.
inline Bytes value_prefix() {
    return hex("a1"
               "6556616c7565"              // "Value"
               "a3"
               "6974696d657374616d70" "00"  // "timestamp": 0
               "6470617468" "6161"         // "path": "a"
               "6576616c7565");            // "value"
}

}  // namespace test
