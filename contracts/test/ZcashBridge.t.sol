// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Test} from "forge-std-1.12.0/Test.sol";

import {ZcashBridge} from "src/ZcashBridge.sol";
import {WZec} from "src/WZec.sol";

contract ZcashBridgeTest is Test {
    WZec internal token;
    ZcashBridge internal bridge;

    address internal constant user = address(0xBEEF);

    bytes32 internal currentEthRoot;
    uint64 internal currentEthBlock;
    bytes32 internal currentZecRoot;
    uint64 internal currentZecBlock;
    bool internal stateInitialized;

    function setUp() public {
        token = new WZec();
        bridge = new ZcashBridge(address(token));
        token.setBridge(address(bridge));
    }

    function test_SubmitStateUpdate_MintsWZec() public {
        uint256 amount = 25e8;
        ZcashBridge.ProcessedZecToEthTransfer[] memory mintTransfers = _singleMint(user, amount);
        ZcashBridge.ProcessedEthToZecTransfer[] memory burnTransfers = _emptyBurns();

        _applyStateUpdate(mintTransfers, burnTransfers);

        assertEq(token.balanceOf(user), amount, "Mint did not credit recipient");
        assertEq(token.totalSupply(), amount, "Incorrect total supply");
    }

    function test_RequestWithdrawal_LocksTokens() public {
        uint256 amount = 10e8;
        _applyStateUpdate(_singleMint(user, amount), _emptyBurns());

        bytes20 pubkeyHash = bytes20(keccak256(abi.encodePacked(user)));
        vm.startPrank(user);
        token.approve(address(bridge), amount);
        uint256 requestId = bridge.requestWithdrawal(amount, pubkeyHash);
        vm.stopPrank();

        ZcashBridge.WithdrawalRequest memory request = bridge.getWithdrawalRequest(requestId);
        assertEq(request.requester, user, "Requester mismatch");
        assertEq(request.amount, amount, "Amount mismatch");
        assertEq(request.processed, false, "Premature processing");
        assertEq(token.balanceOf(user), 0, "User balance should be zero");
        assertEq(token.balanceOf(address(bridge)), amount, "Bridge should hold locked tokens");
        assertEq(bridge.totalLocked(), amount, "totalLocked mismatch");
    }

    function test_SubmitStateUpdate_ProcessesWithdrawal() public {
        uint256 amount = 5e8;
        _applyStateUpdate(_singleMint(user, amount), _emptyBurns());

        bytes20 pubkeyHash = bytes20(keccak256(abi.encodePacked(user)));
        vm.startPrank(user);
        token.approve(address(bridge), amount);
        uint256 requestId = bridge.requestWithdrawal(amount, pubkeyHash);
        vm.stopPrank();

        ZcashBridge.ProcessedEthToZecTransfer[] memory burns = new ZcashBridge.ProcessedEthToZecTransfer[](1);
        burns[0] = ZcashBridge.ProcessedEthToZecTransfer({amount: amount, pubkeyHash: pubkeyHash});

        _applyStateUpdate(_emptyMints(), burns);

        ZcashBridge.WithdrawalRequest memory request = bridge.getWithdrawalRequest(requestId);
        assertEq(request.processed, true, "Withdrawal was not processed");
        assertEq(token.balanceOf(address(bridge)), 0, "Bridge should not keep burned tokens");
        assertEq(token.totalSupply(), 0, "Supply should shrink after burn");
        assertEq(bridge.totalLocked(), 0, "Locked total should decrease");
        assertEq(bridge.totalBurned(), amount, "Burn stats incorrect");
    }

    function test_RevertWhen_StateMismatch() public {
        _applyStateUpdate(_singleMint(user, 1e8), _emptyBurns());

        ZcashBridge.StateUpdate memory badUpdate = ZcashBridge.StateUpdate({
            previousEthRoot: bytes32(uint256(123)),
            previousEthBlockNumber: currentEthBlock,
            newEthRoot: bytes32(uint256(333)),
            newEthBlockNumber: currentEthBlock + 1,
            previousZecRoot: currentZecRoot,
            previousZecBlockNumber: currentZecBlock,
            newZecRoot: bytes32(uint256(444)),
            newZecBlockNumber: currentZecBlock + 1,
            zecToEthTransfers: _emptyMints(),
            ethToZecTransfers: _emptyBurns()
        });

        vm.expectRevert(ZcashBridge.InvalidPreviousState.selector);
        bridge.submitStateUpdate(badUpdate);
    }

    function testFuzz_RequestWithdrawal(uint96 fuzzAmount, bytes20 pubkeyHash) public {
        vm.assume(pubkeyHash != bytes20(0));
        uint256 amount = bound(uint256(fuzzAmount), 1, type(uint96).max);

        _applyStateUpdate(_singleMint(user, amount), _emptyBurns());

        vm.startPrank(user);
        token.approve(address(bridge), amount);
        bridge.requestWithdrawal(amount, pubkeyHash);
        vm.stopPrank();

        assertEq(token.balanceOf(address(bridge)), amount, "Bridge should hold locked tokens");
        assertEq(bridge.totalLocked(), amount, "Locked total mismatch");
    }

    function _singleMint(address recipient, uint256 amount)
        internal
        pure
        returns (ZcashBridge.ProcessedZecToEthTransfer[] memory transfers)
    {
        transfers = new ZcashBridge.ProcessedZecToEthTransfer[](1);
        transfers[0] = ZcashBridge.ProcessedZecToEthTransfer({amount: amount, to: recipient});
    }

    function _emptyMints() internal pure returns (ZcashBridge.ProcessedZecToEthTransfer[] memory transfers) {
        transfers = new ZcashBridge.ProcessedZecToEthTransfer[](0);
    }

    function _emptyBurns() internal pure returns (ZcashBridge.ProcessedEthToZecTransfer[] memory transfers) {
        transfers = new ZcashBridge.ProcessedEthToZecTransfer[](0);
    }

    function _applyStateUpdate(
        ZcashBridge.ProcessedZecToEthTransfer[] memory mintTransfers,
        ZcashBridge.ProcessedEthToZecTransfer[] memory burnTransfers
    ) internal {
        ZcashBridge.StateUpdate memory update = ZcashBridge.StateUpdate({
            previousEthRoot: stateInitialized ? currentEthRoot : bytes32(0),
            previousEthBlockNumber: stateInitialized ? currentEthBlock : 0,
            newEthRoot: keccak256(
                abi.encode(currentEthRoot, mintTransfers.length, burnTransfers.length, block.timestamp)
            ),
            newEthBlockNumber: stateInitialized ? currentEthBlock + 1 : 1,
            previousZecRoot: stateInitialized ? currentZecRoot : bytes32(0),
            previousZecBlockNumber: stateInitialized ? currentZecBlock : 0,
            newZecRoot: keccak256(abi.encode(currentZecRoot, block.number, mintTransfers.length, burnTransfers.length)),
            newZecBlockNumber: stateInitialized ? currentZecBlock + 1 : 1,
            zecToEthTransfers: mintTransfers,
            ethToZecTransfers: burnTransfers
        });

        bridge.submitStateUpdate(update);

        currentEthRoot = update.newEthRoot;
        currentEthBlock = update.newEthBlockNumber;
        currentZecRoot = update.newZecRoot;
        currentZecBlock = update.newZecBlockNumber;
        stateInitialized = true;
    }
}
