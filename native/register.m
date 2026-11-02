#import <Foundation/Foundation.h>
#import <Carbon/Carbon.h>
int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 2) { fprintf(stderr, "Usage: register /path/to/KeyIn.app\n"); return 2; }
        NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[1]]];
        OSStatus status = TISRegisterInputSource((__bridge CFURLRef)url);
        printf("Registration status: %d\n", (int)status);
        NSArray *sources = CFBridgingRelease(TISCreateInputSourceList(NULL, true));
        printf("Total system sources: %lu\n", (unsigned long)sources.count);
        BOOL enabled = NO;
        for (id entry in sources) {
            TISInputSourceRef source = (__bridge TISInputSourceRef)entry;
            NSString *name = (__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyLocalizedName);
            NSString *identifier = (__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
            if (![identifier containsString:@"keyin"] && ![name containsString:@"KeyIn"]) continue;
            OSStatus result = TISEnableInputSource(source);
            CFBooleanRef isEnabled = TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled);
            CFBooleanRef selectable = TISGetInputSourceProperty(source, kTISPropertyInputSourceIsSelectCapable);
            printf("%s [%s]: enable=%d enabled=%d selectable=%d\n", name.UTF8String, identifier.UTF8String, (int)result, isEnabled && CFBooleanGetValue(isEnabled), selectable && CFBooleanGetValue(selectable));
            if (result == noErr && isEnabled && CFBooleanGetValue(isEnabled) && selectable && CFBooleanGetValue(selectable)) enabled = YES;
        }
        return status == noErr && enabled ? 0 : 1;
    }
}
