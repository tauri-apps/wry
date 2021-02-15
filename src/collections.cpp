#include "collections.h"

#define WIN32_LEAN_AND_MEAN
#include <Windows.h>
#include <winrt/Windows.Foundation.Collections.h>
using namespace winrt;
void* ivector(const char* js) {
    auto x = winrt::single_threaded_vector<hstring>({winrt::to_hstring(js)});

    return reinterpret_cast<void*>(winrt::detach_abi(x));
}

HWND hidden_parent;
LRESULT CALLBACK windowProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_DESTROY:
            CloseWindow(hidden_parent);
            PostQuitMessage(0);
            break;
        default:
            return DefWindowProc(hwnd, msg, wParam, lParam);
    }
    return 0;
}

void SkipTaskbar(HWND window) {
    LPTSTR className;
    GetClassName(window, className, 100);

    hidden_parent = CreateWindowEx(0, className, (LPCSTR) "Hidden Parent", NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

    SetWindowLongPtr(window, GWLP_HWNDPARENT, (LONG_PTR)hidden_parent);
    SetWindowLong(window, GWL_EXSTYLE,
                  GetWindowLong(window, GWL_EXSTYLE) & ~WS_EX_APPWINDOW & ~WS_EX_TOOLWINDOW);
    SetWindowLongPtr(window, GWLP_WNDPROC, (LONG_PTR)&windowProc);
}
