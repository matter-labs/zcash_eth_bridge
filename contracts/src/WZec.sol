// contracts/GLDToken.sol
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin-contracts-5.0.2/interfaces/IERC20.sol";
import {IERC20Metadata} from "@openzeppelin-contracts-5.0.2/interfaces/IERC20Metadata.sol";


/// @title WZec
/// @notice Minimal ERC20 compatible token that represents wrapped Zcash on Ethereum.
/// @dev Minting and burning are restricted to a single bridge contract that is set once by the owner.
contract WZec is IERC20, IERC20Metadata {
    /// @notice Token name.
    string public constant name = "Wrapped Zcash";

    /// @notice Token symbol.
    string public constant symbol = "WZEC";

    /// @notice Token decimals follow native Zcash precision (8 decimals).
    uint8 public constant decimals = 8;

    /// @notice Total token supply.
    uint256 public totalSupply;

    /// @notice Mapping of user balances.
    mapping(address => uint256) public balanceOf;

    /// @notice Allowance mapping following ERC20 semantics.
    mapping(address => mapping(address => uint256)) public allowance;

    /// @notice Current token owner allowed to perform administrative actions.
    address public owner;

    /// @notice Bridge contract that is allowed to mint and burn tokens.
    address public bridge;

    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event BridgeUpdated(address indexed newBridge);

    error Unauthorized();
    error BridgeAlreadySet();
    error ZeroAddress();
    error InsufficientBalance();
    error InsufficientAllowance();

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    modifier onlyBridge() {
        if (msg.sender != bridge) revert Unauthorized();
        _;
    }

    constructor() {
        owner = msg.sender;
        emit OwnershipTransferred(address(0), msg.sender);
    }

    /// @notice Transfer ownership of the token contract.
    /// @param newOwner Address of the new owner.
    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) revert ZeroAddress();
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }

    /// @notice Permanently set the bridge contract allowed to mint and burn tokens.
    /// @param newBridge Address of the bridge contract.
    function setBridge(address newBridge) external onlyOwner {
        if (newBridge == address(0)) revert ZeroAddress();
        if (bridge != address(0)) revert BridgeAlreadySet();
        bridge = newBridge;
        emit BridgeUpdated(newBridge);
    }

    /// @notice Transfer tokens to another address.
    /// @param to Recipient.
    /// @param amount Amount to transfer.
    /// @return True if the transfer succeeded.
    function transfer(address to, uint256 amount) external returns (bool) {
        _transfer(msg.sender, to, amount);
        return true;
    }

    /// @notice Approve a spender to transfer tokens on behalf of the caller.
    /// @param spender Address allowed to spend.
    /// @param amount Allowance amount.
    /// @return True if the approval succeeded.
    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    /// @notice Transfer tokens using an allowance.
    /// @param from Address to pull tokens from.
    /// @param to Recipient address.
    /// @param amount Amount to transfer.
    /// @return True if the transfer succeeded.
    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 allowed = allowance[from][msg.sender];
        if (allowed < amount) revert InsufficientAllowance();
        if (allowed != type(uint256).max) {
            unchecked {
                allowance[from][msg.sender] = allowed - amount;
            }
            emit Approval(from, msg.sender, allowance[from][msg.sender]);
        }
        _transfer(from, to, amount);
        return true;
    }

    /// @notice Mint new tokens to a recipient. Callable only by the bridge.
    /// @param to Recipient that will receive the newly minted tokens.
    /// @param amount Amount to mint.
    function mint(address to, uint256 amount) external onlyBridge {
        if (to == address(0)) revert ZeroAddress();
        totalSupply += amount;
        unchecked {
            balanceOf[to] += amount;
        }
        emit Transfer(address(0), to, amount);
    }

    /// @notice Burn tokens from the bridge's balance after they are locked on-chain.
    /// @param amount Amount to burn.
    function burn(uint256 amount) external onlyBridge {
        if (balanceOf[msg.sender] < amount) revert InsufficientBalance();
        unchecked {
            balanceOf[msg.sender] -= amount;
        }
        totalSupply -= amount;
        emit Transfer(msg.sender, address(0), amount);
    }

    function _transfer(address from, address to, uint256 amount) internal {
        if (to == address(0)) revert ZeroAddress();
        if (balanceOf[from] < amount) revert InsufficientBalance();
        unchecked {
            balanceOf[from] -= amount;
            balanceOf[to] += amount;
        }
        emit Transfer(from, to, amount);
    }
}
