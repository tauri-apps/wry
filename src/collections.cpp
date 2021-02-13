#include "collections.h"

#define WIN32_LEAN_AND_MEAN
#include <Windows.h>
#include <winrt/Windows.Foundation.Collections.h>
using namespace winrt;
void *ivector(const char *js)
{
    auto x = winrt::single_threaded_vector<hstring>({winrt::to_hstring(js)});

    return reinterpret_cast<void *>(winrt::detach_abi(x));
}

void SkipTaskbar(HWND window){

    LPTSTR className;
    GetClassName(window, className, 100);

    const HWND parent = CreateWindowEx(0, className, (LPCSTR) "Hidden Parent", NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);

    SetWindowLongPtr(window, GWLP_HWNDPARENT, (LONG_PTR)parent);
    SetWindowLong(window, GWL_EXSTYLE,
                  GetWindowLong(window, GWL_EXSTYLE) & ~WS_EX_APPWINDOW & ~WS_EX_TOOLWINDOW);

}
