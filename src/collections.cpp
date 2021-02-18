#include "collections.h"

#define WIN32_LEAN_AND_MEAN
#include <Windows.h>
#include <ShObjIdl_core.h>
#include <winrt/Windows.Foundation.Collections.h>
using namespace winrt;
void* ivector(const char* js) {
    auto x = winrt::single_threaded_vector<hstring>({winrt::to_hstring(js)});

    return reinterpret_cast<void*>(winrt::detach_abi(x));
}

void SkipTaskbar(HWND window) {
    ITaskbarList* taskbar;
    if (CoCreateInstance(CLSID_TaskbarList, NULL, CLSCTX_SERVER, IID_ITaskbarList, (LPVOID*)&taskbar) == S_OK)
        taskbar->DeleteTab(window);

    taskbar->Release();
}
