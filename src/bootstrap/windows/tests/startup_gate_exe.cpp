#include <cstdio>
extern "C" __declspec(dllimport) int GateVerified();
int main() {
    if (!GateVerified()) return 2;
    std::puts("Game entrypoint ran after preparation; original instructions restored.");
    return 0;
}
