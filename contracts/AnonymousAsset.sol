pragma solidity ^0.5.0;
pragma experimental ABIEncoderV2;

import "./ReportsHandle.sol";
import "./utils/Secp256k1.sol";

// Consider: Avoid inheritting
contract AnonymousAsset is ReportsHandle {
    event StoreCiphertext(bytes ciphertext);

    // Encrypted states
    mapping(uint256 => bytes[]) private _ciphertexts;
    // Store lock parameters to avoid form data collision.
    mapping(uint256 => mapping (bytes32 => bytes32)) private _lockParams;

    constructor(
        bytes memory _report,
        bytes memory _reportSig
    ) ReportsHandle(_report, _reportSig) public { }

    // Register a new TEE participant.
    function register(bytes memory _report, bytes memory _reportSig) public {
        handleReport(_report, _reportSig);
    }

    // Store ciphertexts which is generated by trusted environment.
    function stateTransition(
        uint256 _stateId,
        bytes[] memory _newCiphertexts,
        bytes32[] memory _newLockParams,
        bytes memory _enclaveSig
    ) public {
        uint256 param_len = _newLockParams.length;
        require(param_len == _newCiphertexts.length, "Invalid parameter length.");

        address inpEnclaveAddr = Secp256k1.recover(_newLockParams[0], _enclaveSig);
        require(enclaveAddress[inpEnclaveAddr] == inpEnclaveAddr, "Invalid enclave signature.");

        for (uint32 i = 0; i < param_len; i++) {
            require(_lockParams[_stateId][_newLockParams[i]] == 0, "The state has already been modified.");

             _lockParams[_stateId][_newLockParams[i]] = _newLockParams[i];
            _ciphertexts[_stateId].push(_newCiphertexts[i]);

            // Emit event over iterations because ABIEncoderV2 is not supported web3-rust.
            emit StoreCiphertext(_newCiphertexts[i]);
        }
    }
}
