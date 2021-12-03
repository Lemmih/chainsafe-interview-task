pragma solidity >=0.4.24;

contract SimpleStorage {
    string _cid;

    constructor(string memory cid) public {
        _cid = cid;
    }

    function getCID() view public returns (string memory) {
        return _cid;
    }

    function setCID(string memory cid) public {
        _cid = cid;
    }
}
