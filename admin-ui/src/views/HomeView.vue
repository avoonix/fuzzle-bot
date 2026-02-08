<script setup lang="ts">
import { ref, watch } from 'vue';
import { useFetch } from '@vueuse/core';
import { useCounterStore } from '@/stores/counter';

const url = ref('/api/pending-sets')

interface StickerSetPub {
    id: string,
    title?: string,
}

const { data, error, execute: refetch } = useFetch(url, { refetch: true, updateDataOnError: true }).json<StickerSetPub[]>()

watch(data, () => console.log(data))

const counter = useCounterStore();

</script>

<template>
  <main>
    <div class="d-flex">
      <div>
    <TheWelcome />
    <v-btn to="/bans">
      bans page
    </v-btn>
    <v-btn to="/about">
      about view
    </v-btn>
    <span @click="counter.increment()">
    {{ counter.count }}
</span>
    <v-text-field v-model="url" />
      <div v-if="data">
        <div v-for="set of data">
          <img :src="`/thumbnails/sticker-set/${set.id}/image.png`" loading="lazy" width="128" height="128" />
          {{ set }}
          <v-btn :to="{name: 'banSetView', params: {setId: set.id}}">
            ban set view
          </v-btn>
        </div>

      </div>
    <v-btn color="primary" @click="refetch()">
      Button
    </v-btn>
    {{ error }}
    </div>

    <router-view />
</div>
  </main>
</template>
