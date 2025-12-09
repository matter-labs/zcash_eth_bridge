// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {WZec} from "src/WZec.sol";

/// @title ZcashBridge
/// @notice Handles minting and burning of WZec based on cross-chain state updates between Ethereum and Zcash.
contract ZcashBridge {
    /// @dev Zcash-to-Ethereum transfer that mints WZec to the specified address.
    struct ProcessedZecToEthTransfer {
        uint256 amount;
        address to;
    }

    /// @dev Ethereum-to-Zcash transfer that burns locked WZec corresponding to a pubkey hash.
    struct ProcessedEthToZecTransfer {
        uint256 amount;
        bytes20 pubkeyHash;
    }

    /// @dev Complete state update submitted by bridge operators.
    struct StateUpdate {
        bytes32 previousEthRoot;
        uint64 previousEthBlockNumber;
        bytes32 newEthRoot;
        uint64 newEthBlockNumber;
        bytes32 previousZecRoot;
        uint64 previousZecBlockNumber;
        bytes32 newZecRoot;
        uint64 newZecBlockNumber;
        ProcessedZecToEthTransfer[] zecToEthTransfers;
        ProcessedEthToZecTransfer[] ethToZecTransfers;
    }

    /// @dev Bridge state checkpoints for both chains.
    struct BridgeState {
        bytes32 ethRoot;
        uint64 ethBlockNumber;
        bytes32 zecRoot;
        uint64 zecBlockNumber;
    }

    /// @dev User request to withdraw to Zcash.
    struct WithdrawalRequest {
        address requester;
        uint256 amount;
        bytes20 pubkeyHash;
        bool processed;
    }

    error InvalidPreviousState();
    error InvalidBlockNumber();
    error ZeroAmount();
    error EmptyPubkeyHash();
    error InvalidRecipient();
    error WithdrawalNotFound(bytes32 key);
    error WithdrawalAlreadyProcessed(uint256 requestId);

    event StateUpdated(
        bytes32 previousEthRoot,
        bytes32 newEthRoot,
        uint64 previousEthBlockNumber,
        uint64 newEthBlockNumber,
        bytes32 previousZecRoot,
        bytes32 newZecRoot,
        uint64 previousZecBlockNumber,
        uint64 newZecBlockNumber
    );
    event WithdrawalRequested(uint256 indexed requestId, address indexed requester, uint256 amount, bytes20 pubkeyHash);
    event WithdrawalProcessed(uint256 indexed requestId, uint256 amount, bytes20 pubkeyHash);
    event ZecTransferProcessed(address indexed recipient, uint256 amount);

    WZec public immutable token;

    BridgeState public latestState;
    bool public stateInitialized;
    uint256 public nextWithdrawalId = 1;
    uint256 public totalLocked;
    uint256 public totalMinted;
    uint256 public totalBurned;

    mapping(uint256 => WithdrawalRequest) private withdrawalRequests;
    mapping(bytes32 => uint256[]) private pendingWithdrawalIds;
    mapping(bytes32 => uint256) private pendingWithdrawalIndex;

    constructor(address tokenAddress) {
        if (tokenAddress == address(0)) revert InvalidRecipient();
        token = WZec(tokenAddress);
    }

    /// @notice Compute the key that groups withdrawal requests by amount and lock script.
    /// @param amount Requested withdrawal amount.
    /// @param pubkeyHash Recipient pubkey hash on Zcash.
    /// @return Withdrawal grouping key.
    function computeWithdrawalKey(uint256 amount, bytes20 pubkeyHash) public pure returns (bytes32) {
        return keccak256(abi.encode(amount, pubkeyHash));
    }

    /// @notice Retrieve details about a withdrawal request.
    /// @param requestId Withdrawal identifier.
    /// @return request Withdrawal request metadata.
    function getWithdrawalRequest(uint256 requestId) external view returns (WithdrawalRequest memory request) {
        request = withdrawalRequests[requestId];
    }

    /// @notice Retrieve the most recent state checkpoint for both chains.
    /// @return state Latest bridge state.
    function getLatestState() external view returns (BridgeState memory state) {
        state = latestState;
    }

    /// @notice Number of pending withdrawals for a given key.
    /// @param key Withdrawal grouping key (amount + pubkey hash).
    /// @return count Pending withdrawal count for the key.
    function pendingWithdrawalCount(bytes32 key) external view returns (uint256) {
        uint256[] storage queue = pendingWithdrawalIds[key];
        return queue.length - pendingWithdrawalIndex[key];
    }

    /// @notice Request withdrawal to the Zcash chain by locking WZec tokens.
    /// @param amount Amount of WZec to withdraw.
    /// @param pubkeyHash Recipient pubkey hash on Zcash.
    /// @return requestId Identifier of the newly created request.
    function requestWithdrawal(uint256 amount, bytes20 pubkeyHash) external returns (uint256 requestId) {
        if (amount == 0) revert ZeroAmount();
        if (pubkeyHash.length == 0) revert EmptyPubkeyHash();

        token.transferFrom(msg.sender, address(this), amount);
        totalLocked += amount;

        requestId = nextWithdrawalId++;
        withdrawalRequests[requestId] =
            WithdrawalRequest({requester: msg.sender, amount: amount, pubkeyHash: pubkeyHash, processed: false});

        bytes32 key = computeWithdrawalKey(amount, pubkeyHash);
        pendingWithdrawalIds[key].push(requestId);

        emit WithdrawalRequested(requestId, msg.sender, amount, pubkeyHash);
    }

    /// @notice Submit a state update along with processed cross-chain transfers.
    /// @param update Full state update payload.
    function submitStateUpdate(StateUpdate calldata update) external {
        _validateStateTransition(update);

        _processZecToEthTransfers(update.zecToEthTransfers);
        _processEthToZecTransfers(update.ethToZecTransfers);
    }

    function _validateStateTransition(StateUpdate calldata update) internal {
        if (stateInitialized) {
            if (
                update.previousEthRoot != latestState.ethRoot
                    || update.previousEthBlockNumber != latestState.ethBlockNumber
                    || update.previousZecRoot != latestState.zecRoot
                    || update.previousZecBlockNumber != latestState.zecBlockNumber
            ) {
                revert InvalidPreviousState();
            }
        } else {
            stateInitialized = true;
        }

        if (update.newEthBlockNumber <= update.previousEthBlockNumber) revert InvalidBlockNumber();
        if (update.newZecBlockNumber <= update.previousZecBlockNumber) revert InvalidBlockNumber();

        emit StateUpdated(
            update.previousEthRoot,
            update.newEthRoot,
            update.previousEthBlockNumber,
            update.newEthBlockNumber,
            update.previousZecRoot,
            update.newZecRoot,
            update.previousZecBlockNumber,
            update.newZecBlockNumber
        );

        latestState = BridgeState({
            ethRoot: update.newEthRoot,
            ethBlockNumber: update.newEthBlockNumber,
            zecRoot: update.newZecRoot,
            zecBlockNumber: update.newZecBlockNumber
        });
    }

    function _processZecToEthTransfers(ProcessedZecToEthTransfer[] calldata transfers) internal {
        uint256 length = transfers.length;
        for (uint256 i; i < length; ++i) {
            ProcessedZecToEthTransfer calldata transferData = transfers[i];
            address recipient = transferData.to;
            uint256 amount = transferData.amount;
            if (recipient == address(0)) revert InvalidRecipient();
            if (amount == 0) revert ZeroAmount();
            token.mint(recipient, amount);
            totalMinted += amount;
            emit ZecTransferProcessed(recipient, amount);
        }
    }

    function _processEthToZecTransfers(ProcessedEthToZecTransfer[] calldata transfers) internal {
        uint256 length = transfers.length;
        for (uint256 i; i < length; ++i) {
            ProcessedEthToZecTransfer calldata transferData = transfers[i];
            if (transferData.amount == 0) revert ZeroAmount();
            if (transferData.pubkeyHash == bytes20(0)) revert EmptyPubkeyHash();
            uint256 requestId = _popNextWithdrawal(transferData.amount, transferData.pubkeyHash);
            WithdrawalRequest storage request = withdrawalRequests[requestId];
            if (request.processed) revert WithdrawalAlreadyProcessed(requestId);
            request.processed = true;

            totalLocked -= request.amount;
            totalBurned += request.amount;
            token.burn(request.amount);

            emit WithdrawalProcessed(requestId, request.amount, request.pubkeyHash);
        }
    }

    function _popNextWithdrawal(uint256 amount, bytes20 pubkeyHash) internal returns (uint256 requestId) {
        bytes32 key = computeWithdrawalKey(amount, pubkeyHash);
        uint256 cursor = pendingWithdrawalIndex[key];
        uint256[] storage queue = pendingWithdrawalIds[key];
        if (cursor >= queue.length) revert WithdrawalNotFound(key);
        requestId = queue[cursor];
        pendingWithdrawalIndex[key] = cursor + 1;
    }
}
