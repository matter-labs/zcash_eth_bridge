// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std-1.12.0/Test.sol";

import {ZcashBridge} from "src/ZcashBridge.sol";
import {WZec} from "src/WZec.sol";

contract ZcashBridgeInvariantTest is Test {
    WZec internal token;
    ZcashBridge internal bridge;
    ZcashBridgeHandler internal handler;

    function setUp() external {
        token = new WZec();
        bridge = new ZcashBridge(address(token));
        token.setBridge(address(bridge));

        handler = new ZcashBridgeHandler(bridge, token);

        targetContract(address(handler));
    }

    function invariant_LockedBalanceMatchesBridgeBalance() external view {
        assertEq(
            token.balanceOf(address(bridge)), bridge.totalLocked(), "Bridge locked total must equal escrowed balance"
        );
    }
}

contract ZcashBridgeHandler is Test {
    ZcashBridge internal immutable bridge;
    WZec internal immutable token;

    bytes32 internal currentEthRoot;
    uint64 internal currentEthBlock;
    bytes32 internal currentZecRoot;
    uint64 internal currentZecBlock;
    bool internal stateInitialized;
    uint256 internal nonce;

    address[] internal actors;

    constructor(ZcashBridge bridge_, WZec token_) {
        bridge = bridge_;
        token = token_;

        for (uint256 i; i < 3; ++i) {
            actors.push(makeAddr(string(abi.encodePacked("actor", i))));
        }
    }

    function mintFromZec(uint96 amountSeed, uint256 actorSeed) external {
        uint256 amount = bound(uint256(amountSeed), 1, 10_000 * 1e8);
        address recipient = actors[bound(actorSeed, 0, actors.length - 1)];
        ZcashBridge.ProcessedZecToEthTransfer[] memory mints = new ZcashBridge.ProcessedZecToEthTransfer[](1);
        mints[0] = ZcashBridge.ProcessedZecToEthTransfer({amount: amount, to: recipient});
        _submitStateUpdate(mints, _emptyBurns());
    }

    function requestWithdrawal(uint256 amountSeed, uint256 actorSeed) external {
        address actor = actors[bound(actorSeed, 0, actors.length - 1)];
        uint256 balance = token.balanceOf(actor);
        if (balance == 0) return;

        uint256 amount = bound(amountSeed, 1, balance);
        bytes20 pubkeyHash = bytes20(keccak256(abi.encodePacked(actor, amount)));

        vm.startPrank(actor);
        token.approve(address(bridge), amount);
        bridge.requestWithdrawal(amount, pubkeyHash);
        vm.stopPrank();
    }

    function processWithdrawal(uint256 requestId) external {
        ZcashBridge.WithdrawalRequest memory request = bridge.getWithdrawalRequest(requestId);
        if (request.amount == 0 || request.processed) return;

        ZcashBridge.ProcessedEthToZecTransfer[] memory burns = new ZcashBridge.ProcessedEthToZecTransfer[](1);
        burns[0] = ZcashBridge.ProcessedEthToZecTransfer({amount: request.amount, pubkeyHash: request.pubkeyHash});

        _submitStateUpdate(_emptyMints(), burns);
    }

    function _submitStateUpdate(
        ZcashBridge.ProcessedZecToEthTransfer[] memory mints,
        ZcashBridge.ProcessedEthToZecTransfer[] memory burns
    ) internal {
        ZcashBridge.StateUpdate memory update =
            ZcashBridge.StateUpdate({
                previousEthRoot: stateInitialized ? currentEthRoot : bytes32(0),
                previousEthBlockNumber: stateInitialized ? currentEthBlock : 0,
                newEthRoot: keccak256(abi.encode(currentEthRoot, ++nonce, mints.length, burns.length)),
                newEthBlockNumber: stateInitialized ? currentEthBlock + 1 : 1,
                previousZecRoot: stateInitialized ? currentZecRoot : bytes32(0),
                previousZecBlockNumber: stateInitialized ? currentZecBlock : 0,
                newZecRoot: keccak256(abi.encode(currentZecRoot, nonce, block.timestamp)),
                newZecBlockNumber: stateInitialized ? currentZecBlock + 1 : 1,
                zecToEthTransfers: mints,
                ethToZecTransfers: burns
            });

        bridge.submitStateUpdate(update);

        currentEthRoot = update.newEthRoot;
        currentEthBlock = update.newEthBlockNumber;
        currentZecRoot = update.newZecRoot;
        currentZecBlock = update.newZecBlockNumber;
        stateInitialized = true;
    }

    function _emptyMints() internal pure returns (ZcashBridge.ProcessedZecToEthTransfer[] memory arr) {
        arr = new ZcashBridge.ProcessedZecToEthTransfer[](0);
    }

    function _emptyBurns() internal pure returns (ZcashBridge.ProcessedEthToZecTransfer[] memory arr) {
        arr = new ZcashBridge.ProcessedEthToZecTransfer[](0);
    }
}
