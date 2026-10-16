#include <sonde/sonde.hpp>

#include <cfloat>
#include <cmath>
#include <cstring>

namespace sonde {

namespace {

/// The half precision bits of `value` if it is one exactly.
bool to_half(float value, std::uint16_t& half) {
    std::uint32_t bits;
    std::memcpy(&bits, &value, sizeof bits);
    const auto sign = static_cast<std::uint16_t>((bits >> 16) & 0x8000u);
    const std::uint32_t exponent = (bits >> 23) & 0xffu;
    const std::uint32_t mantissa = bits & 0x7fffffu;

    if (exponent == 0 && mantissa == 0) {  // zero
        half = sign;
        return true;
    }
    if (exponent == 0 || exponent == 0xff) {  // subnormal floats are too small, inf, NaN
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
    std::uint16_t half;
    if (std::isnan(value)) {
        half = 0x7e00;
    } else if (std::isinf(value)) {
        half = value > 0 ? 0x7c00 : 0xfc00;
    } else if (!to_half(value, half)) {
        check(cbor_encode_float(cbor_, value));
        return;
    }
    check(cbor_encode_half_float(cbor_, &half));
}

void Encoder::floating(double value) {
    if (!std::isfinite(value)) {
        floating(static_cast<float>(value));
    } else if (std::fabs(value) <= FLT_MAX &&
               static_cast<double>(static_cast<float>(value)) == value) {
        floating(static_cast<float>(value));
    } else {
        check(cbor_encode_double(cbor_, value));
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

void Encoder::check(CborError error) {
    // Running out of space is no error here: tinycbor goes on counting the bytes needed,
    // and the tracer decides whether the message fits.
    if (error != CborNoError && error != CborErrorOutOfMemory && *error_ == CborNoError) {
        *error_ = error;
    }
}

}  // namespace sonde
