pragma solidity ^0.4.13;


/// Prefixing all methods with _ so that the names don't collide with the builtins.
contract Builtins {

  event _Sha256Event(bytes32 hash);
  event _EcrecoverEvent(address addr);
  event _ModexpEvent(bytes modexp);

  function _sha256(bytes input) public returns (bytes32) {
    return sha256(input);
  }

  function _sha256Event(bytes input) public {
    emit _Sha256Event(sha256(input));
  }

  function _ecrecover(bytes32 h, uint8 v, bytes32 r, bytes32 s) public returns (address) {
    return ecrecover(h, v, r, s);
  }

  function _ecrecoverEvent(bytes32 h, uint8 v, bytes32 r, bytes32 s) public {
    emit _EcrecoverEvent(ecrecover(h, v, r, s));
  }

  function _modexp(
    bytes memory base,
    bytes memory exponent,
    bytes memory modulus
  ) public returns (bytes) {
    var (success, ret) = ModexpPrecompile.modexp(base, exponent, modulus);
    return ret;
  }

  function _modexpEvent(
    bytes memory base,
    bytes memory exp,
    bytes memory mod
  ) public {
    bytes memory ret = _modexp(base, exp, mod);
    emit _ModexpEvent(ret);
  }
}

/// Wrapper for built-in BigNumber_modexp (contract 0x5).
/// See https://github.com/ethereum/EIPs/pull/198 and
/// https://gist.github.com/axic/6ae83f0ab7ee2e8e69f4c240c5b90de8
library ModexpPrecompile {
  function modexp(bytes base, bytes exponent, bytes modulus) internal returns (bool success, bytes output) {
    uint base_length = base.length;
    uint exponent_length = exponent.length;
    uint modulus_length = modulus.length;

    uint size = (32 * 3) + base_length + exponent_length + modulus_length;
    bytes memory input = new bytes(size);
    output = new bytes(modulus_length);

    assembly {
      mstore(add(input, 32), base_length)
      mstore(add(input, 64), exponent_length)
      mstore(add(input, 96), modulus_length)
    }

    BytesTool.memcopy(base, 0, input, 96, base_length);
    BytesTool.memcopy(exponent, 0, input, 96 + base_length, exponent_length);
    BytesTool.memcopy(modulus, 0, input, 96 + base_length + exponent_length, modulus_length);

    assembly {
      success := call(gas(), 5, 0, add(input, 32), size, add(output, 32), modulus_length)
    }
  }
}

library BytesTool {
  function memcopy(bytes src, uint srcoffset, bytes dst, uint dstoffset, uint len) pure internal {
    assembly {
      src := add(src, add(32, srcoffset))
      dst := add(dst, add(32, dstoffset))

      // copy 32 bytes at once
      for {}
      iszero(lt(len, 32))
      {
        dst := add(dst, 32)
        src := add(src, 32)
        len := sub(len, 32)
      }
      { mstore(dst, mload(src)) }

      // copy the remainder (0 < len < 32)
      let mask := sub(exp(256, sub(32, len)), 1)
      let srcpart := and(mload(src), not(mask))
      let dstpart := and(mload(dst), mask)
      mstore(dst, or(srcpart, dstpart))
    }
  }
}
