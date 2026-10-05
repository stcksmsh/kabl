/* Measurement-only CLAP proxy. Never install this instead of the product.
 * ABI fields follow clap-sys 0.5.0 / CLAP plugin, entry and factory declarations.
 * Fixed storage; callback does no allocation, logging or locking.
 * Execution interval encloses original process plus clock edge. Other proxy work
 * is outside that interval and calibrated separately by PROXY_BENCH.
 */
#include <stdint.h>
#include <stdbool.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <dlfcn.h>
struct plugin;
struct version { uint32_t major, minor, revision; };
struct plugin {
 const void *desc; void *data;
 bool (*init)(const struct plugin *);
 void (*destroy)(const struct plugin *);
 bool (*activate)(const struct plugin *,double,uint32_t,uint32_t);
 void (*deactivate)(const struct plugin *);
 bool (*start)(const struct plugin *);
 void (*stop)(const struct plugin *);
 void (*reset)(const struct plugin *);
 int32_t (*process)(const struct plugin *,const void *);
 const void *(*extension)(const struct plugin *,const char *);
 void (*main)(const struct plugin *);
};
struct factory {
 uint32_t (*count)(const struct factory *);
 const void *(*descriptor)(const struct factory *,uint32_t);
 const struct plugin *(*create)(const struct factory *,const void *,const char *);
};
struct entry { struct version version; bool (*init)(const char *); void (*deinit)(void); const void *(*factory)(const char *); };
struct event_header {uint32_t size,time;uint16_t space,type;uint32_t flags;};
struct input_events {void *ctx;uint32_t (*size)(const struct input_events *);const struct event_header *(*get)(const struct input_events *,uint32_t);};
struct transport_info {struct event_header header;uint32_t flags;int64_t beats,seconds;double tempo,tempo_inc;};
struct process_info {int64_t steady;uint32_t frames;const void *transport,*inputs,*outputs;uint32_t input_count,output_count;const struct input_events *events;const void *out_events;};
static const struct entry *real_entry;
static const struct factory *real_factory;
struct param_info {uint32_t id,flags;void *cookie;char name[256],module[1024];double min,max,value;};
struct params {uint32_t (*count)(const struct plugin *);bool (*info)(const struct plugin *,uint32_t,struct param_info *);bool (*value)(const struct plugin *,uint32_t,double *);};
struct slot {
 _Atomic(const struct plugin *) plugin;
 int32_t (*process)(const struct plugin *,const void *);
 void (*destroy)(const struct plugin *);
 atomic_uint_fast64_t count, total, maximum, histogram[1000], frames;
 void (*reset)(const struct plugin *); bool (*start)(const struct plugin *);
 struct {unsigned kind;uint64_t frame;int64_t steady;uint64_t ns;} trace[128]; atomic_uint trace_count;
 struct {uint64_t frame;uint32_t frames,flags;int64_t beats,seconds;double tempo,host;} positions[32768]; unsigned position_count;uint32_t last_flags;
 struct {uint64_t frame;uint32_t offset,flags;int64_t beats,seconds;double tempo;} updates[4096];unsigned update_count;
 const struct params *params;uint32_t host_id;
 bool note_seen;
 uint64_t previous_begin, arrival_count, arrival_sum, arrival_min, arrival_max, processed_frames;
 uint32_t minimum_frames, maximum_frames;
 const void *(*extension)(const struct plugin *,const char *);
 struct {bool (*hard)(const struct plugin *); bool (*set)(const struct plugin *,int32_t);} render;
};
static struct slot slots[16];
static atomic_uint used; static bool reset_trace; static bool suppress_render_restart;
static uint64_t now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return (uint64_t)t.tv_sec*1000000000+t.tv_nsec; }
static struct slot *find(const struct plugin *p) { for(unsigned i=0;i<atomic_load_explicit(&used,memory_order_acquire);i++) if(atomic_load_explicit(&slots[i].plugin,memory_order_relaxed)==p) return &slots[i]; abort(); }
static void trace(struct slot *s,unsigned kind,int64_t steady) {
 unsigned i=atomic_fetch_add(&s->trace_count,1);if(i<128){s->trace[i].kind=kind;s->trace[i].frame=atomic_load(&s->frames);s->trace[i].steady=steady;s->trace[i].ns=now();}
}
static void reset(const struct plugin *p){struct slot *s=find(p);trace(s,1,0);s->note_seen=false;s->reset(p);}
static bool start(const struct plugin *p){struct slot *s=find(p);trace(s,2,0);s->note_seen=false;return s->start(p);}
static int32_t timed(const struct plugin *p,const void *input) {
 struct slot *s=find(p);
 if(reset_trace) {
  const struct process_info *info=input;
  if(!s->note_seen&&info->events&&info->events->size&&info->events->get) {
   uint32_t n=info->events->size(info->events);
   for(uint32_t i=0;i<n&&i<2048;i++) {
    const struct event_header *h=info->events->get(info->events,i);
    if(h&&h->space==0&&h->type==10&&h->size>=21) {
     const unsigned char *bytes=(const unsigned char *)h;
     if((bytes[18]&0xf0)==0x90&&bytes[20]) {trace(s,3,h->time);s->note_seen=true;break;}
    }
   }
  }
  atomic_fetch_add(&s->frames,info->frames);
 }
 uint64_t start=now();
 const struct process_info *info=input;
 const struct transport_info *t=info->transport;
 uint32_t flags=t?t->flags:0;
 if(s->position_count<32768) {
  unsigned i=s->position_count++;s->positions[i].frame=s->processed_frames;s->positions[i].frames=info->frames;s->positions[i].flags=flags;
  s->params->value(p,s->host_id,&s->positions[i].host);
  if(t){s->positions[i].beats=t->beats;s->positions[i].seconds=t->seconds;s->positions[i].tempo=t->tempo;}
 }
 if(info->events&&info->events->size&&info->events->get){uint32_t n=info->events->size(info->events);for(uint32_t j=0;j<n&&j<2048;j++){const struct event_header *h=info->events->get(info->events,j);if(h&&h->space==0&&h->type==9&&s->update_count<4096){const struct transport_info *u=(const void *)h;unsigned i=s->update_count++;s->updates[i].frame=s->processed_frames;s->updates[i].offset=h->time;s->updates[i].flags=u->flags;s->updates[i].beats=u->beats;s->updates[i].seconds=u->seconds;s->updates[i].tempo=u->tempo;}}}
 s->last_flags=flags;
 s->processed_frames+=info->frames;
 if(!s->minimum_frames||info->frames<s->minimum_frames)s->minimum_frames=info->frames;
 if(info->frames>s->maximum_frames)s->maximum_frames=info->frames;
 if(s->previous_begin){uint64_t interval=start-s->previous_begin;s->arrival_count++;s->arrival_sum+=interval;if(!s->arrival_min||interval<s->arrival_min)s->arrival_min=interval;if(interval>s->arrival_max)s->arrival_max=interval;}
 s->previous_begin=start;
 uint64_t measured=now(); int32_t result=s->process(p,input); uint64_t ns=now()-measured;
 atomic_fetch_add_explicit(&s->count,1,memory_order_relaxed);
 atomic_fetch_add_explicit(&s->total,ns,memory_order_relaxed);
 uint64_t old=atomic_load_explicit(&s->maximum,memory_order_relaxed);
 if(ns>old) atomic_store_explicit(&s->maximum,ns,memory_order_relaxed); /* host serializes instance callbacks */
 atomic_fetch_add_explicit(&s->histogram[ns/10000<999?ns/10000:999],1,memory_order_relaxed);
 return result;
}
static void destroy(const struct plugin *p) {
 struct slot *s=find(p);
 const char *path=getenv("KABL_TIMING_OUTPUT"); FILE *f=path?fopen(path,"a"):NULL;
 if(f) {
  uint64_t n=atomic_load(&s->count),sum=atomic_load(&s->total),max=atomic_load(&s->maximum), cumulative=0,p99=0;
  for(unsigned i=0;i<1000;i++){ cumulative+=atomic_load(&s->histogram[i]); if(n&&cumulative>=(n*99+99)/100){p99=(i+1)*10000;break;} }
  fprintf(f,"%u,%lu,%lu,%lu,%lu,%lu,%u,%u,%lu,%lu,%lu,%lu\n",(unsigned)(s-slots),(unsigned long)n,(unsigned long)sum,(unsigned long)max,(unsigned long)p99,(unsigned long)s->processed_frames,s->minimum_frames,s->maximum_frames,(unsigned long)s->arrival_count,(unsigned long)s->arrival_min,(unsigned long)s->arrival_max,(unsigned long)s->arrival_sum); fclose(f);
  if(reset_trace){char name[4096];snprintf(name,sizeof name,"%s.trace",path);f=fopen(name,"a");if(f){unsigned n=atomic_load(&s->trace_count);for(unsigned i=0;i<n&&i<128;i++)fprintf(f,"%u,%u,%lu,%ld,%lu\n",(unsigned)(s-slots),s->trace[i].kind,(unsigned long)s->trace[i].frame,(long)s->trace[i].steady,(unsigned long)s->trace[i].ns);fclose(f);}}
 }
 if(path){char name[4096];snprintf(name,sizeof name,"%s.transport",path);f=fopen(name,"a");if(f){for(unsigned i=0;i<s->position_count;i++)fprintf(f,"%u,%lu,%u,%u,%ld,%ld,%.9f,%.1f\n",(unsigned)(s-slots),(unsigned long)s->positions[i].frame,s->positions[i].frames,s->positions[i].flags,(long)s->positions[i].beats,(long)s->positions[i].seconds,s->positions[i].tempo,s->positions[i].host);fclose(f);}}
 if(path){char name[4096];snprintf(name,sizeof name,"%s.updates",path);f=fopen(name,"a");if(f){for(unsigned i=0;i<s->update_count;i++)fprintf(f,"%u,%lu,%u,%u,%ld,%ld,%.9f\n",(unsigned)(s-slots),(unsigned long)s->updates[i].frame,s->updates[i].offset,s->updates[i].flags,(long)s->updates[i].beats,(long)s->updates[i].seconds,s->updates[i].tempo);fclose(f);}}
 s->destroy(p); atomic_store(&s->plugin,NULL);
}
static bool hard(const struct plugin *p) {return find(p)->render.hard(p);}
static bool render_set(const struct plugin *p,int32_t mode){struct slot *s=find(p);trace(s,mode?4:5,0);return suppress_render_restart?(mode==0||mode==1):s->render.set(p,mode);}
static const struct {bool (*hard)(const struct plugin *);bool (*set)(const struct plugin *,int32_t);} wrapped_render={hard,render_set};
static const void *extension(const struct plugin *p,const char *id) {
 struct slot *s=find(p);const void *result=s->extension(p,id);
 if(reset_trace&&result&&!strcmp(id,"clap.render")){memcpy(&s->render,result,sizeof s->render);return &wrapped_render;}return result;
}
static uint32_t count(const struct factory *f) { (void)f; return real_factory->count(real_factory); }
static const void *descriptor(const struct factory *f,uint32_t i) { (void)f;return real_factory->descriptor(real_factory,i); }
static const struct plugin *create(const struct factory *f,const void *host,const char *id) {
 (void)f; unsigned index=atomic_load(&used);if(index==16) return NULL;
 const struct plugin *p=real_factory->create(real_factory,host,id); if(!p)return NULL;
 struct slot *s=&slots[index];atomic_store(&s->plugin,p);s->process=p->process;s->destroy=p->destroy;s->reset=p->reset;s->start=p->start;s->extension=p->extension;s->params=s->extension(p,"clap.params");struct param_info info={0};if(s->params && s->params->info(p,17,&info))s->host_id=info.id;atomic_store_explicit(&used,index+1,memory_order_release);
 ((struct plugin *)p)->extension=extension;((struct plugin *)p)->reset=reset;((struct plugin *)p)->start=start;((struct plugin *)p)->process=timed;((struct plugin *)p)->destroy=destroy;return p;
}
static struct factory wrapped={count,descriptor,create};
static bool init(const char *path) {
 reset_trace=getenv("KABL_RESET_TRACE")!=NULL;suppress_render_restart=getenv("KABL_NO_RENDER_RESTART")!=NULL; const char *target=getenv("KABL_TIMING_PLUGIN");if(!target)return false;
 void *lib=dlopen(target,RTLD_NOW|RTLD_LOCAL);if(!lib)return false;
 real_entry=dlsym(lib,"clap_entry");return real_entry&&real_entry->init(path);
}
static void deinit(void) { if(real_entry)real_entry->deinit(); }
static const void *factory(const char *id) {
 const void *f=real_entry->factory(id);
 if(f&&!strcmp(id,"clap.plugin-factory")){real_factory=f;return &wrapped;}return f;
}
__attribute__((visibility("default"))) const struct entry clap_entry={{1,2,2},init,deinit,factory};

#ifdef PROXY_BENCH
__attribute__((noinline)) static int32_t noop(const struct plugin *p,const void *v){(void)p;(void)v;__asm__ volatile("" ::: "memory");return 0;}
int main(void) {
 struct plugin p={0};slots[0].process=noop;atomic_store(&slots[0].plugin,&p);atomic_store(&used,1);
 struct process_info info={.frames=256};
 const unsigned count=100000;uint64_t first=now();for(unsigned i=0;i<count;i++)noop(&p,&info);uint64_t direct=now()-first;
 first=now();for(unsigned i=0;i<count;i++)timed(&p,&info);uint64_t proxy=now()-first;
 printf("{\"iterations\":%u,\"direct_ns_per_call\":%.3f,\"proxy_ns_per_call\":%.3f,\"added_ns_per_call\":%.3f}\n",count,(double)direct/count,(double)proxy/count,(double)(proxy-direct)/count);return 0;
}
#endif
