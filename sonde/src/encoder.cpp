#include <sonde/sonde.hpp>

#include <cstring>

namespace sonde {

namespace {

/// The half precision bits of the float with the bits `bits` if it is one exactly, any
/// NaN as the quiet one.
bool to_half(std::uint32_t bits, std::uint16_t& half) {
    const auto sign = static_cast<std::uint16_t>((bits >> 16) & 0x8000u);
    const std::uint32_t exponent = (bits >> 23) & 0xffu;
    const std::uint32_t mantissa = bits & 0x7fffffu;

    if (exponent == 0 && mantissa == 0) {  // zero
        half = sign;
        return true;
    }
    if (exponent == 0xff) {  // infinity or NaN
        half = mantissa != 0 ? 0x7e00 : static_cast<std::uint16_t>(sign | 0x7c00u);
        return true;
    }
    if (exponent == 0) {  // subnormal floats are too small
        return false;
    }
    const int power = static_cast<int>(exponent) - 127;
    if (power >= -14 && power <= 15) {  // a normal half
        if ((mantissa & 0x1fffu) != 0) {
            return false;
        }
        half = static_cast<std::uint16_t>(sign | (static_cast<std::uint32_t>(power + 15) << 10) |
                                          (mantissa >> 13));
        return true;
    }
    if (power >= -24 && power < -14) {  // a subnormal half, a multiple of 2^-24
        const std::uint32_t significand = mantissa | 0x800000u;
        const int shift = -(power + 1);
        if ((significand & ((std::uint32_t{1} << shift) - 1)) != 0) {
            return false;
        }
        half = static_cast<std::uint16_t>(sign | (significand >> shift));
        return true;
    }
    return false;
}

/// The single precision bits of the double with the bits `bits` if it is one exactly,
/// or infinite or NaN. On the bits, as comparing and converting doubles takes library
/// calls on a target without an FPU for them.
bool to_single(std::uint64_t bits, std::uint32_t& single) {
    const auto sign = static_cast<std::uint32_t>(bits >> 63) << 31;
    const auto exponent = static_cast<std::uint32_t>(bits >> 52) & 0x7ffu;
    const std::uint64_t mantissa = bits & ((std::uint64_t{1} << 52) - 1);

    if (exponent == 0x7ff) {  // infinity or NaN
        single = sign | 0x7f800000u | (mantissa != 0 ? 0x400000u : 0u);
        return true;
    }
    if (exponent == 0) {  // zero, subnormal doubles are too small
        single = sign;
        return mantissa == 0;
    }
    const int power = static_cast<int>(exponent) - 1023;
    if (power >= -126 && power <= 127) {  // a normal float, with 23 of the 52 bits
        if ((mantissa & 0x1fffffffu) != 0) {
            return false;
        }
        single = sign | (static_cast<std::uint32_t>(power + 127) << 23) |
                 static_cast<std::uint32_t>(mantissa >> 29);
        return true;
    }
    if (power >= -149 && power < -126) {  // a subnormal float, a multiple of 2^-149
        const std::uint64_t significand = mantissa | (std::uint64_t{1} << 52);
        const int shift = -(power + 97);
        if ((significand & ((std::uint64_t{1} << shift) - 1)) != 0) {
            return false;
        }
        single = sign | static_cast<std::uint32_t>(significand >> shift);
        return true;
    }
    return false;
}

/// Writes the float with the bits `bits` as a half if that keeps it.
CborError encode_single(CborEncoder* cbor, std::uint32_t bits) {
    std::uint16_t half;
    if (to_half(bits, half)) {
        return cbor_encode_half_float(cbor, &half);
    }
    return cbor_encode_floating_point(cbor, CborFloatType, &bits);
}

}  // namespace

void Encoder::null() {
    check(cbor_encode_null(cbor_));
}

void Encoder::boolean(bool value) {
    check(cbor_encode_boolean(cbor_, value));
}

void Encoder::uint(std::uint64_t value) {
    check(cbor_encode_uint(cbor_, value));
}

void Encoder::sint(std::int64_t value) {
    check(cbor_encode_int(cbor_, value));
}

// Floats take the shortest width that keeps the value, as serde_cbor writes them.
void Encoder::floating(float value) {
    std::uint32_t bits;
    std::memcpy(&bits, &value, sizeof bits);
    check(encode_single(cbor_, bits));
}

void Encoder::floating(double value) {
    std::uint64_t bits;
    std::memcpy(&bits, &value, sizeof bits);
    std::uint32_t single;
    if (to_single(bits, single)) {
        check(encode_single(cbor_, single));
    } else {
        check(cbor_encode_floating_point(cbor_, CborDoubleType, &bits));
    }
}

void Encoder::string(const char* data, std::size_t size) {
    check(cbor_encode_text_string(cbor_, data, size));
}

bool Encoder::open_map(CborEncoder& map, std::size_t pairs) {
    const CborError error = cbor_encoder_create_map(cbor_, &map, pairs);
    check(error);
    return error == CborNoError || error == CborErrorOutOfMemory;
}

bool Encoder::open_array(CborEncoder& array, std::size_t elements) {
    const CborError error = cbor_encoder_create_array(cbor_, &array, elements);
    check(error);
    return error == CborNoError || error == CborErrorOutOfMemory;
}

void Encoder::close(const CborEncoder& container) {
    check(cbor_encoder_close_container(cbor_, &container));
}

namespace detail {

void encode_labels(Encoder& encoder, Span<Label> labels) {
    CborEncoder map;
    if (!encoder.open_map(map, labels.size)) {
        return;
    }
    Encoder entries{&map, encoder.error_};
    for (std::size_t i = 0; i < labels.size; ++i) {
        entries.sint(labels.data[i].value);
        entries.string(labels.data[i].text.data(), labels.data[i].text.size());
    }
    encoder.close(map);
}

}  // namespace detail

void Encoder::check(CborError error) {
    // Running out of space is no error here: tinycbor goes on counting the bytes needed,
    // and the tracer decides whether the message fits.
    if (error != CborNoError && error != CborErrorOutOfMemory && *error_ == CborNoError) {
        *error_ = error;
    }
}

}  // namespace sonde
