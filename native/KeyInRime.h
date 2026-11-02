#import <Foundation/Foundation.h>

// Main-thread-only adapter. Every input client owns an isolated Rime session.
@interface KeyInRime : NSObject
+ (BOOL)startWithSharedData:(NSString *)shared userData:(NSString *)user;
+ (NSString *)version;
@property(nonatomic, readonly) BOOL available;
@property(nonatomic) BOOL learning;
@property(nonatomic, readonly) NSString *input;
@property(nonatomic, readonly) NSUInteger caret;
- (BOOL)setInput:(NSString *)raw;
- (BOOL)processKey:(int)key;
- (NSDictionary *)snapshot;
- (NSString *)selectCandidate:(NSUInteger)index;
- (void)clear;
@end
