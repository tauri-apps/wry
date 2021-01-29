#include "collections.h"

#define WIN32_LEAN_AND_MEAN
#include <winrt/Windows.Foundation.Collections.h>
using namespace winrt;
void* ivector(const char* js) {
    auto x = winrt::single_threaded_vector<hstring>({ winrt::to_hstring(js) });

    return reinterpret_cast<void*>(winrt::detach_abi(x));
}